#![allow(dead_code)]
use std::alloc::Layout;

/// Calculate layout for `T` using the inner value's layout
pub(crate) fn memory_layout_for_t<T>(layout: Layout) -> Layout {
    // Calculate layout using the given value layout.
    Layout::new::<T>().extend(layout).unwrap().0.pad_to_align()
}

pub(crate) fn is_dangling<T: ?Sized>(ptr: *const T) -> bool {
    ptr.cast::<()>().addr() == usize::MAX
}
