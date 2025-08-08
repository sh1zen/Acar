use crate::WatchGuard;
use std::any::Any;

pub trait Downcast {
    fn try_downcast_ref<U: Any>(&self) -> Option<&U>;

    fn try_downcast_mut<'a, U: Any>(&'a mut self) -> Option<WatchGuard<'a, U>>;

    fn downcast_ref<U: Any>(&self) -> &U {
        match self.try_downcast_ref::<U>() {
            Some(data) => &*data,
            None => panic!("Downcast failed"),
        }
    }

    fn downcast_mut<'a, U: Any>(&'a mut self) -> WatchGuard<'a, U> {
        match self.try_downcast_mut::<U>() {
            Some(data) => data,
            None => panic!("Downcast mut failed"),
        }
    }
}
