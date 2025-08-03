#[cfg(test)]

mod tests {
    use crate::{AnyRef, Downcast, WeakAnyRef};
    use std::any::TypeId;
    use std::rc::Rc;
    use std::sync::Barrier;
    use std::sync::atomic::AtomicU8;
    use std::sync::atomic::Ordering::{Acquire, Relaxed};
    use std::thread;

    #[test]
    fn drop_inplace() {
        use std::{rc::Rc, thread};

        let mut handles = vec![];
        let mut val = AnyRef::new(Rc::new(String::from("hello")));
        for _ in 0..100 {
            let mut val_clone = val.clone();
            handles.push(thread::spawn(move || {
                let mut rc = val_clone.downcast_mut::<Rc<String>>();
                for _ in 0..1000 {
                    let mut_ref = Rc::get_mut(&mut rc);
                    if let Some(s) = mut_ref {
                        s.push_str(":1");
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        println!(
            "val: {:?}",
            val.downcast_ref::<Rc<String>>().split(":").count()
        );
    }

    #[test]
    fn new_and_type() {
        let x = AnyRef::new(42u32);
        assert_eq!(x.inner().type_id, TypeId::of::<u32>());
        assert_eq!(
            AnyRef::new(String::new()).inner().type_id,
            TypeId::of::<String>()
        );
        assert_eq!(
            AnyRef::new(Box::new(String::new())).inner().type_id,
            TypeId::of::<Box<String>>()
        );
    }

    #[test]
    fn test_strong_weak_counts() {
        let x = AnyRef::new("hello");
        let y = x.clone();
        {
            let j = y.clone();
            assert_eq!(AnyRef::weak_count(&x), 0);
            assert_eq!(AnyRef::strong_count(&j), 3);
        }
        let x_d = x.downgrade();
        let weak_clone = x_d.clone();
        assert_eq!(AnyRef::strong_count(&x), 2);
        assert_eq!(AnyRef::weak_count(&x), 2);

        drop(x);
        let x = weak_clone.upgrade().unwrap();
        drop(weak_clone);
        assert_eq!(AnyRef::strong_count(&x), 2);
        assert_eq!(AnyRef::weak_count(&x), 1);
    }

    #[test]
    fn test_downgrade_and_upgrade() {
        let x = AnyRef::new("test");
        let weak = x.downgrade();

        assert!(weak.upgrade().is_some());
        assert!(weak.upgrade().unwrap().downcast_ref::<&str>().eq(&"test"));
    }

    #[test]
    fn test_downcast_success() {
        let x = AnyRef::new(1234i64);
        let val = x.downcast_ref::<i64>();
        assert_eq!(*val, 1234);
    }

    #[test]
    #[should_panic(expected = "Downcast failed")]
    fn test_downcast_fail() {
        let x = AnyRef::new(1234i64);
        let _: &u32 = x.downcast_ref(); // Panics
    }

    #[test]
    fn test_try_downcast_mut_fail_with_multiple_strong() {
        let rc = Rc::new(0u32);

        let fake_send = AnyRef::new(rc.clone());
        let fake_send2 = AnyRef::new(rc.clone());

        // Thread 1: clona l'Rc
        let handle1 = thread::spawn({
            move || {
                for _ in 0..10000 {
                    let _ = fake_send.downcast_ref::<Rc<u32>>().clone(); // accesso concorrente al contatore
                }
            }
        });

        // Thread 2: clona l'Rc
        let handle2 = thread::spawn(move || {
            for _ in 0..10000 {
                let _ = fake_send2.downcast_ref::<Rc<u32>>().clone(); // accesso concorrente al contatore
            }
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // Alla fine, drop finale su Rc
        drop(rc);
    }

    #[test]
    fn test_try_downcast_mut_success() {
        let mut x = AnyRef::new(42);
        let clone = x.clone().downgrade();
        let y = x.try_downcast_mut::<i32>();
        assert!(y.is_some());
        if let Some(mut x) = y {
            *x += 1;
            assert_eq!(*x, 43);
        };

        assert_eq!(*clone.downcast_ref::<i32>(), 43);
    }

    #[test]
    fn test_weak_drops_when_no_strong() {
        let weak: WeakAnyRef;
        {
            let x = AnyRef::new(42);
            weak = x.downgrade();
            assert!(weak.upgrade().is_some());
        }
        assert_eq!(weak.clone().strong_count(), 0);

        let _x = weak.clone();
        assert_eq!(weak.clone().weak_count(), 3);

        // After drop, weak cannot upgrade
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_default_fill() {
        let x: AnyRef = Default::default();
        let x = AnyRef::fill(x, 10i32);
        assert_eq!(x.downcast_ref::<i32>().clone(), 10i32);

        struct Def {
            data: String,
        }

        impl Default for Def {
            fn default() -> Self {
                Def {
                    data: String::from("hello"),
                }
            }
        }

        let x = AnyRef::default_with::<Def>();
        assert_eq!(x.downcast_ref::<Def>().data, String::from("hello"));
    }

    #[test]
    fn test_from_raw_in_reconstruction() {
        let x = AnyRef::new(String::from("hello"));
        let raw = AnyRef::into_raw(x);
        let y = AnyRef::from_raw(raw);
        let val = y.downcast_ref::<String>();
        assert_eq!(val, &"hello");
    }

    #[test]
    fn test_drop() {
        struct Foo;
        static DROP_COUNTER: AtomicU8 = AtomicU8::new(0);
        impl Drop for Foo {
            fn drop(&mut self) {
                DROP_COUNTER.fetch_add(1, Relaxed);
            }
        }
        let foo = AnyRef::new(Foo);
        {
            let _x = AnyRef::clone(&foo);
            let _weak_foo = AnyRef::downgrade(&foo);
        }
        let weak_foo = AnyRef::downgrade(&foo);
        let other_weak_foo = WeakAnyRef::clone(&weak_foo);

        drop(weak_foo); // Doesn't do anything
        drop(foo); // drop data here

        assert!(other_weak_foo.upgrade().is_none());
        assert_eq!(DROP_COUNTER.load(Acquire), 1);
    }

    #[test]
    fn test_concurrent_clone_and_drop() {
        let x = AnyRef::new(123i32);
        let mut handles = vec![];
        let barrier = AnyRef::new(Barrier::new(10));

        for i in 0..10 {
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

    #[test]
    fn test_concurrent_downgrade_and_upgrade() {
        let x = AnyRef::new("abc");
        let weak = x.downgrade();
        let mut handles = vec![];

        for _ in 0..10 {
            let weak_clone = weak.clone();
            handles.push(thread::spawn(move || {
                let upgraded = weak_clone.upgrade();
                if let Some(v) = upgraded {
                    assert_eq!(v.downcast_ref::<&str>(), &"abc");
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert!(weak.upgrade().is_some());
    }

    #[test]
    fn test_acar_thread_safe() {
        let mut x = AnyRef::new(String::from("hello"));
        let mut y = x.clone();

        if let Some(mut v) = x.try_downcast_mut::<String>() {
            v.push_str(":1");
        }

        if let Some(mut v) = y.try_downcast_mut::<String>() {
            v.push_str(":2");
        }

        assert_eq!(*y.downcast_ref::<String>(), "hello:1:2");

        let weak = x.downgrade();
        let mut handles = vec![];

        for i in 0..10 {
            let mut acar_clone = AnyRef::clone(&x);
            handles.push(thread::spawn(move || {
                let mut val = acar_clone.downcast_mut::<String>();
                val.push_str(format!(":{}", i).as_str());
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            weak.upgrade()
                .unwrap()
                .downcast_ref::<String>()
                .split(":")
                .count(),
            13
        );
    }

    #[test]
    fn test_threaded_upgrade_after_drop() {
        let weak_holder: WeakAnyRef;
        {
            let x = AnyRef::new("persistent");
            weak_holder = x.downgrade();

            let x2 = x.clone();

            thread::spawn(move || {
                let val = x2.downcast_ref::<&str>();
                assert_eq!(*val, "persistent");
                // Drop happens when thread ends
            })
            .join()
            .unwrap();
        }
        // All strong refs dropped, weak is now invalid
        assert!(weak_holder.upgrade().is_none());
    }

    #[test]
    fn test_try_unwrap() {
        let x = AnyRef::new(String::from("hello"));
        let y = AnyRef::try_unwrap::<String>(x).unwrap();
        assert_eq!(y, "hello");
    }
}
