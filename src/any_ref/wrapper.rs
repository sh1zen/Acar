use crate::WatchGuard;
use crate::any_ref::downcast::Downcast;
use crate::any_ref::inner::{AnyRefInner, MAX_REFCOUNT};
use crate::any_ref::ptr_interface::PtrInterface;
use crate::any_ref::weak::WeakAnyRef;
use crate::utils::is_dangling;
use std::any::{Any, TypeId};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::process::abort;
use std::ptr::NonNull;
use std::sync::atomic;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::{fmt, hint, ptr};

#[repr(C)]
pub struct AnyRef {
    ptr: NonNull<AnyRefInner>,
    phantom: PhantomData<AnyRefInner>,
}

unsafe impl Send for AnyRef {}
unsafe impl Sync for AnyRef {}

impl AnyRef {
    /// Creates a new `AnyRef` containing the given value.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new(42);
    /// assert_eq!(a.as_ref::<i32>(), &42);
    /// ```
    pub fn new<T>(value: T) -> Self
    where
        T: Any + Sized,
    {
        unsafe { Self::from_inner(Box::leak(Box::new(AnyRefInner::new(value))).into()) }
    }

    /// Creates a new `AnyRef` from a boxed value.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let boxed = Box::new("hello");
    /// let a = AnyRef::from_box(boxed);
    /// assert_eq!(a.as_ref::<&str>(), &"hello");
    /// ```
    pub fn from_box<T>(src: Box<T>) -> Self
    where
        T: Any + Sized,
    {
        unsafe { Self::from_inner(Box::leak(Box::new(AnyRefInner::from_box(src))).into()) }
    }

    /// Attempts to extract the inner value if there is exactly one strong reference.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new(123i32);
    /// let value = AnyRef::try_unwrap::<i32>(a).unwrap();
    /// assert_eq!(value, 123);
    /// ```
    pub fn try_unwrap<T>(this: Self) -> Result<T, Self> {
        if this
            .inner()
            .strong
            .compare_exchange(1, 0, Relaxed, Relaxed)
            .is_err()
        {
            return Err(this);
        }

        atomic::fence(Acquire);

        let this = ManuallyDrop::new(this);

        let elem: T = unsafe { this.read_data::<T>() };

        // Make a weak pointer to clean up the implicit strong-weak reference
        let _weak = WeakAnyRef { ptr: this.ptr };

        Ok(elem)
    }

    pub(crate) fn inner(&self) -> &AnyRefInner {
        // This unsafety is ok because while this AnyRef is alive we're guaranteed
        // that the inner pointer is valid. Furthermore, we know that the
        // `ArcInner` structure itself is `Sync` because the inner data is
        // `Sync` as well, so we're ok loaning out an immutable pointer to these
        // contents.
        unsafe { self.ptr.as_ref() }
    }

    #[allow(dead_code)]
    fn inner_mut(&mut self) -> &mut AnyRefInner {
        unsafe { self.ptr.as_mut() }
    }

    /// Returns a raw pointer to the contained type, if possible.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new(50);
    /// let ptr = a.as_cast_ptr::<i32>();
    /// assert_eq!(unsafe { *ptr }, 50);
    /// ```
    pub fn as_cast_ptr<T: Any>(self: &Self) -> *const T {
        if self.inner().type_id != TypeId::of::<T>() {
            panic!("AnyRef: wrong cast in as_ref::<{}>()",  std::any::type_name::<T>());
        }
        let ptr = self.as_ptr();
        ptr as *const T
    }

    /// Returns a reference to the inner value as `&dyn Any`.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new(String::from("hello"));
    /// let any = a.as_ref_any();
    /// assert_eq!(any.downcast_ref::<String>().unwrap(), "hello");
    /// ```
    pub fn as_ref_any(self: &Self) -> &dyn Any {
        let ptr = self.as_ptr();
        unsafe { &*(ptr) }
    }

    /// Returns a reference to the inner value of type `T`.
    ///
    /// # Panics
    /// Panics if the type does not match `T`.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new(3.14f32);
    /// let f = a.as_ref::<f32>();
    /// assert_eq!(*f, 3.14);
    /// ```
    pub fn as_ref<T: Any>(self: &Self) -> &T {
        let ptr = self.as_cast_ptr::<T>();
        unsafe { &*(ptr) }
    }

    /// Returns `true` if the `AnyRef` is the only strong reference to the value.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new("unique");
    /// assert!(AnyRef::is_unique(&a));
    /// let b = a.clone();
    /// assert!(!AnyRef::is_unique(&a));
    /// ```
    pub fn is_unique(this: &Self) -> bool {
        // lock the weak pointer count if we appear to be the sole weak pointer
        // holder.
        //
        // The acquire label here ensures a happens-before relationship with any
        // writes to `strong` (in particular in `Weak::upgrade`) prior to decrements
        // of the `weak` count (via `Weak::drop`, which uses release). If the upgraded
        // weak ref was never dropped, the CAS here will fail so we do not care to synchronize.
        if this
            .inner()
            .weak
            .compare_exchange(1, usize::MAX, Acquire, Relaxed)
            .is_ok()
        {
            // This needs to be an `Acquire` to synchronize with the decrement of the `strong`
            // counter in `drop` -- the only access that happens when any but the last reference
            // is being dropped.
            let unique = this.inner().strong.load(Acquire) == 1;

            // The release write here synchronizes with a read in `downgrade`,
            // effectively preventing the above read of `strong` from happening
            // after the write.
            this.inner().weak.store(1, Release); // release the lock
            unique
        } else {
            false
        }
    }

    /// Convert into a weak reference
    /// # Example
    ///
    /// ```
    /// use crate::castbox::AnyRef;
    /// let five = AnyRef::new(5);
    /// let weak_five = AnyRef::downgrade(&five);
    /// ```
    pub fn downgrade(&self) -> WeakAnyRef {
        // This Relaxed is OK because we're checking the value in the CAS
        // below.
        let mut cur = self.inner().weak.load(Relaxed);

        loop {
            // check if the weak counter is currently "locked"; if so, spin.
            if cur == usize::MAX {
                hint::spin_loop();
                cur = self.inner().weak.load(Relaxed);
                continue;
            }

            // We can't allow the refcount to increase much past `MAX_REFCOUNT`.
            assert!(cur <= MAX_REFCOUNT, "INTERNAL OVERFLOW ERROR");

            // NOTE: this code currently ignores the possibility of overflow
            // into usize::MAX; in general both Rc and Arc need to be adjusted
            // to deal with overflow.

            // Unlike with Clone(), we need this to be an Acquire read to
            // synchronize with the write coming from `is_unique`, so that the
            // events prior to that write happen before this read.
            match self
                .inner()
                .weak
                .compare_exchange_weak(cur, cur + 1, Acquire, Relaxed)
            {
                Ok(_) => {
                    // Make sure we do not create a dangling Weak
                    debug_assert!(!is_dangling(self.inner()));
                    return WeakAnyRef { ptr: self.ptr };
                }
                Err(old) => cur = old,
            }
        }
    }

    /// Returns the number of weak references (excluding the implicit one).
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new(10);
    /// let w = a.downgrade();
    /// assert_eq!(AnyRef::weak_count(&a), 1);
    /// ```
    #[inline]
    pub fn weak_count(this: &Self) -> usize {
        let cnt = this.inner().weak.load(Relaxed);
        // If the weak count is currently locked, the value of the
        // count was 0 just before taking the lock.
        if cnt == usize::MAX { 0 } else { cnt - 1 }
    }

    /// Returns the number of strong references.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new("count");
    /// let b = a.clone();
    /// assert_eq!(AnyRef::strong_count(&a), 2);
    /// ```
    #[inline]
    pub fn strong_count(this: &Self) -> usize {
        this.inner().strong.load(Relaxed)
    }

    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        ptr::addr_eq(this.get_ptr().as_ptr(), other.get_ptr().as_ptr())
    }

    pub fn into_raw(self) -> *const Box<dyn Any> {
        let this = ManuallyDrop::new(self);
        let ptr = unsafe { &this.get_ptr().as_mut().data };
        ptr
    }

    pub fn from_raw<T: ?Sized>(ptr: *const T) -> Self {
        unsafe { Self::from_raw_in(ptr) }
    }
}

impl PtrInterface for AnyRef {
    #[inline]
    fn get_ptr(&self) -> NonNull<AnyRefInner> {
        self.ptr
    }

    #[inline]
    unsafe fn from_inner_in(ptr: NonNull<AnyRefInner>) -> Self {
        Self {
            ptr,
            phantom: PhantomData,
        }
    }
}

impl Downcast for AnyRef {
    fn try_downcast_ref<U: Any>(&self) -> Option<&U> {
        if self.inner().type_id == TypeId::of::<U>() {
            match self.inner().get_ref() {
                Some(ptr) => ptr.downcast_ref::<U>(),
                None => None,
            }
        } else {
            None
        }
    }

    fn try_downcast_mut<U: Any>(&mut self) -> Option<WatchGuard<U>> {
        if self.inner().type_id == TypeId::of::<U>() {
            match self.inner().get_mut_ref() {
                Some(ptr) => {
                    let ptr = ptr.downcast_mut::<U>()?;
                    Some(WatchGuard::new(self, ptr))
                }
                None => None,
            }
        } else {
            None
        }
    }
}

impl Clone for AnyRef {
    /// Makes a clone of the `Arc` pointer.
    ///
    /// This creates another pointer to the same allocation, increasing the
    /// strong reference count.
    #[inline]
    fn clone(&self) -> AnyRef {
        // Using a relaxed ordering is alright here, as knowledge of the
        // original reference prevents other threads from erroneously deleting
        // the object.
        let old_size = self.inner().strong.fetch_add(1, Relaxed);

        if old_size > MAX_REFCOUNT {
            abort();
        }

        unsafe { Self::from_inner_in(self.ptr) }
    }
}

impl Default for AnyRef {
    fn default() -> AnyRef {
        unsafe {
            Self::from_inner(
                Box::leak(Box::write(
                    Box::new_uninit(),
                    AnyRefInner::from_box(Box::new(())),
                ))
                .into(),
            )
        }
    }
}

impl AnyRef {
    /// Casts a raw `*const dyn Any` to `&T` if the type matches.
    ///
    /// # Safety
    /// Caller must ensure the pointer is valid and of type `T`.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new(99);
    /// let any_ptr = a.as_ref_any();
    /// let val = AnyRef::cast_raw::<i32>(any_ptr);
    /// assert_eq!(*val.unwrap(), 99);
    /// ```
    pub fn cast_raw<'a, T: 'static>(ptr: *const dyn Any) -> Option<&'a T> {
        unsafe {
            let any_ref = &*ptr;
            any_ref.downcast_ref::<T>()
        }
    }
}

impl AnyRef {
    /// Creates a new `AnyRef` using the default value of `T`.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a: AnyRef = AnyRef::default_with::<String>();
    /// assert_eq!(a.as_ref::<String>(), "");
    /// ```
    pub fn default_with<T: 'static + Default>() -> Self {
        Self::from_box(Box::new(T::default()))
    }

    /// Replaces the inner value with a new value of type `T`.
    ///
    /// # Example
    /// ```
    /// use crate::castbox::AnyRef;
    /// let a = AnyRef::new(0);
    /// let a = AnyRef::fill(a, 123);
    /// assert_eq!(a.as_ref::<i32>(), &123);
    /// ```
    pub fn fill<T: 'static>(this: Self, value: T) -> Self {
        let ref_inner = unsafe { &mut *this.ptr.as_ptr() };
        ref_inner.data = Box::new(value);
        ref_inner.type_id = TypeId::of::<T>();
        this
    }
}

impl Drop for AnyRef {
    fn drop(&mut self) {
        self.inner().lock.unlock();

        // Because `fetch_sub` is already atomic, we do not need to synchronize
        // with other threads unless we are going to delete the object. This
        // same logic applies to the below `fetch_sub` to the `weak` count.
        if self.inner().strong.fetch_sub(1, Release) != 1 {
            return;
        }

        atomic::fence(Acquire);

        let _weak = WeakAnyRef { ptr: self.ptr };

        unsafe { ptr::drop_in_place(&mut (*self.ptr.as_ptr()).data) }
    }
}

impl fmt::Debug for AnyRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("AnyRef::<{}>", self.inner().type_name))
    }
}

impl fmt::Pointer for AnyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.inner().data, f)
    }
}
