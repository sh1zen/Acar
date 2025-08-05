use std::sync::Barrier;
use std::thread;
use castbox::{AnyRef, Downcast};

fn main() {
    let x = AnyRef::new(123i32);
    let mut handles = vec![];
    let barrier = AnyRef::new(Barrier::new(1000));

    for i in 0..1000 {
        let x_clone = x.clone();
        let barrier_clone = barrier.clone();
        handles.push(thread::spawn(move || {
            barrier_clone.downcast_ref::<Barrier>().wait();
            let val = x_clone.downcast_ref::<i32>();
            assert_eq!(*val, 123);
        }));

        assert_eq!(AnyRef::strong_count(&x), i + 2);
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(AnyRef::strong_count(&x), 1);
    assert_eq!(AnyRef::weak_count(&x), 0);
}
