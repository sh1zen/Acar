use crate::mutex::mutex::Mutex;
use std::any::{Any, TypeId};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Acquire;

/// Max number of reference that an any_ref could have
pub const MAX_REFCOUNT: usize = isize::MAX as usize;

/// Actually the main worker of AnyRef
#[repr(C)]
pub struct AnyRefInner {
    pub(crate) data: Box<dyn Any>,
    pub(crate) type_id: TypeId,
    pub(crate) type_name: &'static str,
    pub(crate) lock: Mutex,
    pub(crate) strong: AtomicUsize,
    pub(crate) weak: AtomicUsize,
}

impl AnyRefInner {
    /// Constructs a new `AnyRefInner` from a concrete value.
    /// Internally wraps the value in a `Box<dyn Any>`.
    pub(crate) fn new<T>(value: T) -> Self
    where
        T: Any + Sized,
    {
        Self::from_box(Box::new(value))
    }

    pub(crate) fn from_box<T>(src: Box<T>) -> Self
    where
        T: Any + Sized,
    {
        Self {
            data: src as Box<dyn Any>,
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            lock: Mutex::new(),
            strong: AtomicUsize::new(1),
            weak: AtomicUsize::new(1),
        }
    }

    #[inline(always)]
    pub(crate) fn is_valid(&self) -> bool {
        self.strong.load(Acquire) > 0
    }

    #[inline(always)]
    pub(crate) const fn get_ptr(&self) -> *mut dyn Any {
        // We can just cast the pointer from `UnsafeCell<T>` to `T` because of
        // #[repr(transparent)]. This exploits std's special status, there is
        // no guarantee for user code that this will work in future versions of the compiler!
        &*self.data as *const dyn Any as *mut dyn Any
    }

    pub(crate) fn get_ref(&self) -> Option<&dyn Any> {
        if self.is_valid() {
            unsafe { Some(&*self.get_ptr()) }
        } else {
            None
        }
    }

    pub(crate) fn get_mut_ref(self: &Self) -> Option<&mut dyn Any> {
        if self.is_valid() {
            self.lock.lock();
            unsafe { Some(&mut *self.get_ptr()) }
        } else {
            None
        }
    }
}
