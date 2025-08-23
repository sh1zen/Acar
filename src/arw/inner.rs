use crate::mutex::Mutex;
use std::any::Any;
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::atomic::AtomicUsize;

/// Max number of reference that an any_ref could have
pub(super) const MAX_REFCOUNT: usize = isize::MAX as usize;

/// Actually the main worker
pub(crate) struct ArwInner<T: Sized> {
    pub(crate) lock: Mutex,
    pub(crate) strong: AtomicUsize,
    pub(crate) weak: AtomicUsize,
    pub(crate) val: UnsafeCell<T>,
}

impl<T> ArwInner<T> {
    /// Constructs a new `ArwInner` from a concrete value.
    pub(crate) fn new(val: T) -> Self
    where
        T: Any,
    {
        Self {
            val: UnsafeCell::new(val),
            lock: Mutex::new(),
            strong: AtomicUsize::new(1),
            weak: AtomicUsize::new(1),
        }
    }

    #[inline(always)]
    fn internal_get(&self) -> *mut T {
        self.val.get()
    }

    pub(crate) fn get_ref(&self) -> &T {
        unsafe { &*self.internal_get() }
    }

    pub(crate) fn get_mut_ref(&self) -> &mut T {
        let mut value = unsafe { NonNull::new_unchecked(self.internal_get()) };
        unsafe { &mut *value.as_mut() }
    }
}

impl<T: Default> Default for ArwInner<T> {
    fn default() -> Self {
        Self {
            val: Default::default(),
            lock: Mutex::new(),
            strong: AtomicUsize::new(1),
            weak: AtomicUsize::new(1),
        }
    }
}
