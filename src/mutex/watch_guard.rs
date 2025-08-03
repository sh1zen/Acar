use crate::any_ref::AnyRef;
use std::ops::{Deref, DerefMut};

/// used asa wrapper for a pointer to a reference
#[must_use = "if unused the Mutex will immediately unlock"]
#[clippy::has_significant_drop]
pub struct WatchGuard<'a, T: ?Sized + 'a> {
    data: &'a mut T,
    owner: &'a AnyRef,
}

impl<'mutex, T: ?Sized> WatchGuard<'mutex, T> {
    ///create a new WatchGuard from a &mut T and AnyRef
    pub fn new(owner: &'mutex AnyRef, ptr: &'mutex mut T) -> WatchGuard<'mutex, T> {
        Self { data: ptr, owner }
    }
}

/// `T` must be `Sync` for a [`WatchGuard<T>`] to be `Sync`
/// because it is possible to get a `&T` from `&WatchGuard` (via `Deref`).
unsafe impl<T: ?Sized + Sync> Sync for WatchGuard<'_, T> {}

impl<T: ?Sized> Deref for WatchGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &*self.data
    }
}

impl<T: ?Sized> DerefMut for WatchGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.data
    }
}

impl<T: ?Sized> Drop for WatchGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.owner.inner().lock.unlock();
    }
}
