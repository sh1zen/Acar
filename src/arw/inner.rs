use crate::mutex::Mutex;
use std::any::Any;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Acquire;

/// Max number of reference that an any_ref could have
pub(super) const MAX_REFCOUNT: usize = isize::MAX as usize;

/// Actually the main worker
#[repr(C)]
pub(crate) struct ArwInner<T: Sized> {
    pub(crate) lock: Mutex,
    pub(crate) strong: AtomicUsize,
    pub(crate) weak: AtomicUsize,
    pub(crate) val: T,
}

impl<T> ArwInner<T> {
    /// Constructs a new `ArwInner` from a concrete value.
    pub(crate) fn new(val: T) -> Self
    where
        T: Any,
    {
        Self {
            val,
            lock: Mutex::new(),
            strong: AtomicUsize::new(1),
            weak: AtomicUsize::new(1),
        }
    }

    #[inline(always)]
    fn is_valid(&self) -> bool {
        self.strong.load(Acquire) > 0
    }

    pub(crate) fn get_ref(&self) -> Option<&T> {
        if self.is_valid() {
            Some(&self.val)
        } else {
            None
        }
    }

    pub(crate) fn get_mut_ref(&mut self) -> Option<&mut T> {
        if self.is_valid() {
            Some(&mut self.val)
        } else {
            None
        }
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
