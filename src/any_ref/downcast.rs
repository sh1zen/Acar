use crate::WatchGuardMut;
use crate::mutex::WatchGuardRef;
use std::any::Any;

pub trait Downcast {
    fn try_downcast_ref<U: Any>(&self) -> Option<WatchGuardRef<'_, U>>;

    fn try_downcast_mut<U: Any>(&mut self) -> Option<WatchGuardMut<'_, U>>;

    fn downcast_ref<U: Any>(&self) -> WatchGuardRef<'_, U> {
        match self.try_downcast_ref::<U>() {
            Some(data) => data,
            None => panic!("Downcast failed"),
        }
    }

    fn downcast_mut<U: Any>(&mut self) -> WatchGuardMut<'_, U> {
        match self.try_downcast_mut::<U>() {
            Some(data) => data,
            None => panic!("Downcast mut failed"),
        }
    }
}
