use crate::mutex::Mutex;
use std::any::{Any, TypeId};
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::atomic::AtomicUsize;

/// Max number of reference that an any_ref could have
pub(super) const MAX_REFCOUNT: usize = isize::MAX as usize;

/// Actually the main worker of AnyRef
pub(crate) struct AnyRefInner {
    pub(crate) data: UnsafeCell<Box<dyn Any>>,
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
            data: UnsafeCell::new(src as Box<dyn Any>),
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
            lock: Mutex::new(),
            strong: AtomicUsize::new(1),
            weak: AtomicUsize::new(1),
        }
    }

    #[inline(always)]
    fn internal_get(&self) -> *mut dyn Any {
        let ptr = self.data.get();
        let data = unsafe { &mut **ptr as *mut dyn Any };
        data
    }

    pub(crate) fn get_ref(&self) -> &dyn Any {
        unsafe { &*self.internal_get() }
    }

    pub(crate) fn get_mut_ref(&self) -> &mut dyn Any {
        let mut value = unsafe { NonNull::new_unchecked(self.internal_get()) };
        unsafe { &mut *value.as_mut() }
    }
}

impl Default for AnyRefInner {
    fn default() -> Self {
        Self::from_box(Box::new(()))
    }
}
