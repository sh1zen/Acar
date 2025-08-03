use crate::WatchGuard;
use std::any::Any;

pub trait Downcast {
    fn try_downcast_ref<U: Any>(&self) -> Option<&U>;

    fn try_downcast_mut<U: Any>(&mut self) -> Option<WatchGuard<U>>;

    fn downcast_ref<U: Any>(&self) -> &U {
        match self.try_downcast_ref::<U>() {
            Some(data) => &*data,
            None => panic!("Downcast failed"),
        }
    }

    fn downcast_mut<U: Any>(&mut self) -> WatchGuard<U> {
        match self.try_downcast_mut::<U>() {
            Some(data) => data,
            None => panic!("Downcast mut failed"),
        }
    }
}
