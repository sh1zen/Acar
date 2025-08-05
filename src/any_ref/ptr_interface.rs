#![allow(dead_code)]
use crate::utils::is_dangling;
use crate::any_ref::inner::AnyRefInner;
use std::any::Any;
use std::mem::{offset_of, ManuallyDrop};
use std::ptr;
use std::ptr::NonNull;

pub(crate) trait PtrInterface
where
    Self: Sized,
{
    fn get_non_null_inner(&self) -> NonNull<AnyRefInner>;

    unsafe fn from_inner_in(ptr: NonNull<AnyRefInner>) -> Self;

    #[inline]
    unsafe fn from_ptr_in(ptr: *mut AnyRefInner) -> Self {
        unsafe { Self::from_inner_in(NonNull::new_unchecked(ptr)) }
    }

    fn into_wrapped(self: Self) -> NonNull<AnyRefInner> {
        let this = ManuallyDrop::new(self);
        this.get_non_null_inner()
    }

    unsafe fn read_any(&self) -> Box<dyn Any> {
        unsafe { ptr::read(&self.get_non_null_inner().as_ref().data) }
    }

    unsafe fn read_data<T>(&self) -> T {
        unsafe { ptr::read(self.as_ptr() as *const T) }
    }

    fn as_ptr(&self) -> *const dyn Any {
        let ptr: *mut AnyRefInner = NonNull::as_ptr(self.get_non_null_inner());

        if is_dangling(ptr) {
            // If the pointer is dangling, we return the sentinel directly. This cannot be
            // a valid payload address, as the payload is at least as aligned as AnyRefInner (usize).
            ptr as *const dyn Any
        } else {
            // SAFETY: if is_dangling returns false, then the pointer is dereferenceable.
            // The payload may be dropped at this point, and we have to maintain provenance,
            // so use raw pointer manipulation.
            unsafe { &raw mut *(*ptr).data }
        }
    }

    unsafe fn from_ptr(ptr: *mut AnyRefInner) -> Self {
        unsafe { Self::from_ptr_in(ptr) }
    }

    unsafe fn from_inner(ptr: NonNull<AnyRefInner>) -> Self {
        unsafe { Self::from_inner_in(ptr) }
    }

    #[inline]
    unsafe fn from_raw_in<T: ?Sized>(ptr: *const T) -> Self {
        let inner_ptr = if is_dangling(ptr) {
            // This is a dangling Weak.
            ptr as *mut AnyRefInner
        } else {
            let obj = ptr as *const u8;
            let data_offset = offset_of!(AnyRefInner, data);

            // SAFETY: we assume the dyn Any points to AnyRefInner.data
            unsafe { obj.offset(-(data_offset as isize)) as *mut AnyRefInner }
        };

        unsafe { Self::from_ptr_in(inner_ptr) }
    }
}