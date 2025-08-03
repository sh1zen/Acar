use std::rc::Rc;
use std::thread;
use castbox::{AnyRef, Downcast};

// Wrapper per forzare `Rc<T>` a essere Send (pericoloso!)
struct UnsafeSendRc<T>(Rc<T>);
impl<T> UnsafeSendRc<T> {
  fn new(val: T) -> UnsafeSendRc<T> {
      UnsafeSendRc(Rc::new(val))
  }
}
impl<T> Clone for UnsafeSendRc<T> {
    fn clone(&self) -> Self {
        UnsafeSendRc(self.0.clone())
    }
}
impl <T>Drop for UnsafeSendRc<T>{
    fn drop(&mut self) {
        //println!("{}", Rc::strong_count(&self.0));
    }
}


fn main() {
    let rc = UnsafeSendRc::new(Vec::from([1i32, 2i32, 3i32]));

    let a = AnyRef::new(rc.clone());
    let c = a.downcast_ref::<UnsafeSendRc<Vec<i32>>>().clone();
    let b = AnyRef::new(rc.clone());

    let handle1 = thread::spawn(move || {
        for _ in 0..1_000 {
            let _clone = a.downcast_ref::<UnsafeSendRc<Vec<i32>>>().clone();
            // Simuliamo attività
            std::hint::black_box(&_clone);
        }
        // Il drop avviene automaticamente
    });

    let handle2 = thread::spawn(move || {
        for _ in 0..1_000 {
            let _clone = b.as_ref::<UnsafeSendRc<Vec<i32>>>().clone();
            std::hint::black_box(&_clone);
        }
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    println!("Fatto");
}