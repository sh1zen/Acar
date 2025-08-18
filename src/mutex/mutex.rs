use crate::{AtomicVec, Backoff};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{AtomicU8, AtomicUsize};
use std::thread::Thread;
use std::{fmt, thread};

#[test]
fn test_mutex() {
    use crate::mutex::Mutex;
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;

    let mutex = Mutex::new();

    let m1 = mutex.clone();
    let m2 = mutex.clone();

    mutex.lock_group();
    mutex.lock_group();

    mutex.unlock_group();
    mutex.unlock_group();

    let h1 = thread::spawn(move || {
        m1.lock();
        sleep(Duration::from_millis(100));
        m1.unlock();
    });

    let h2 = thread::spawn(move || {
        m2.lock();
        m2.unlock();
    });

    h1.join().unwrap();
    h2.join().unwrap();

    drop(mutex);
}

#[test]
fn test_mutex2() {
    use crate::mutex::Mutex;
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;

    let mut handles = vec![];

    let mutex = Mutex::new();

    mutex.lock();

    for _i in 0..100 {
        let m1 = mutex.clone();
        handles.push(thread::spawn(move || {
            m1.lock_group();
        }));
    }

    sleep(Duration::from_millis(1000));
    mutex.unlock();

    for h in handles {
        h.join().unwrap();
    }

    for _i in 0..100 {
        mutex.unlock_group();
    }

    drop(mutex);
}

enum MutexType {
    Exclusive,
    Group,
}

/// A fast user space thread locker
type State = u8;

/// unlocked
const UNLOCKED: State = 0;
/// locked, no other threads waiting
const LOCKED: State = 1;
/// multi group lock
const LOCKED_GROUP: State = 3;
/// a dirty state
const DIRTY: State = 4;

struct InnerMutex {
    state: AtomicU8,
    ref_count: AtomicUsize,
    parking_e: AtomicVec<Thread>,
    parking_g: AtomicVec<Thread>,
    locked: AtomicUsize,
}

#[repr(transparent)]
pub struct Mutex {
    ptr: *const InnerMutex,
}

unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

impl Mutex {
    pub fn new() -> Self {
        let ptr = Box::into_raw(Box::new(InnerMutex {
            state: AtomicU8::new(UNLOCKED),
            ref_count: AtomicUsize::new(1),
            parking_e: AtomicVec::new(),
            parking_g: AtomicVec::new(),
            locked: AtomicUsize::new(0),
        }));
        if ptr.is_null() {
            panic!("Happened an invalid allocation for Mutex");
        }
        Self { ptr }
    }

    pub fn get_ref_count(&self) -> usize {
        self.inner().ref_count.load(Acquire)
    }

    #[inline(always)]
    fn inner(&self) -> &InnerMutex {
        unsafe { &*self.ptr }
    }

    pub fn lock(&self) {
        let backoff = Backoff::new();
        let inner = self.inner();

        loop {
            // Spin first to speed things up if the lock is released quickly.
            match self.spin(20) {
                DIRTY => {
                    // if the state is DIRTY and there are no other group waiting is safe to switch to LOCKED
                    if inner.locked.load(Acquire) == 0 {
                        if inner
                            .state
                            .compare_exchange(DIRTY, LOCKED, Acquire, Relaxed)
                            .is_ok()
                        {
                            break;
                        }
                    }
                }
                _ => {
                    if inner
                        .state
                        .compare_exchange(UNLOCKED, LOCKED, Acquire, Relaxed)
                        .is_ok()
                    {
                        break;
                    }
                }
            }

            if backoff.is_completed() {
                self.suspend(MutexType::Exclusive);
            } else {
                backoff.snooze();
            }
        }
    }

    pub fn lock_group(&self) {
        let backoff = Backoff::new();

        // we add it here so that as soon as the lock is available we can proceed to execute
        // all the multi lock group.
        // SAFETY: The unlock will fetch_sub only when the internal state is on LOCKED_GROUP state
        self.inner().locked.fetch_add(1, Acquire);

        loop {
            // Spin first to speed things up if the lock is released quickly.
            match self.spin(20) {
                DIRTY => {
                    if self
                        .inner()
                        .state
                        .compare_exchange(DIRTY, LOCKED_GROUP, Acquire, Relaxed)
                        .is_ok()
                    {
                        self.wake_all(MutexType::Group);
                        break;
                    }
                }
                LOCKED_GROUP => {
                    // if some thread are parked let's wake them up
                    self.wake(MutexType::Group);
                    break;
                }
                _ => {
                    if self
                        .inner()
                        .state
                        .compare_exchange(UNLOCKED, LOCKED_GROUP, Acquire, Relaxed)
                        .is_ok()
                    {
                        // try to wake some thread that maybe are parked but are members of this group
                        self.wake_all(MutexType::Group);
                        break;
                    }
                }
            }

            if backoff.is_completed() {
                self.suspend(MutexType::Group);
            } else {
                backoff.snooze();
            }
        }
    }

    #[inline]
    pub fn is_locked(&self) -> bool {
        self.inner().state.load(Acquire) != UNLOCKED
    }

    #[inline]
    fn spin(&self, mut spin: i32) -> State {
        loop {
            // We only use `load` (and not `swap` or `compare_exchange`)
            // while spinning, to be easier on the caches.
            let state = self.inner().state.load(Relaxed);

            // We stop spinning when the mutex is UNLOCKED
            if state == UNLOCKED || spin == 0 {
                return state;
            }

            std::hint::spin_loop();
            spin -= 1;
        }
    }

    #[inline]
    pub fn unlock_group(&self) {
        if self.inner().state.load(Acquire) != LOCKED_GROUP {
            panic!("Trying to unlock a non Locked Group.");
        }

        if self.inner().locked.fetch_sub(1, Release) == 1 {
            self.inner().state.store(DIRTY, Release);

            // if there are some thread suspended now we must wake them up
            if !self.wake(MutexType::Exclusive) {
                self.wake(MutexType::Group);
            }
        }
    }

    #[inline]
    pub fn unlock(&self) {
        if self
            .inner()
            .state
            .compare_exchange(LOCKED, UNLOCKED, Release, Relaxed)
            .is_err()
        {
            panic!("Is not Locked or is a Locked Group.");
        }

        // if there are some thread suspended now we must wake them up
        if !self.wake(MutexType::Group) {
            self.wake(MutexType::Exclusive);
        }
    }

    #[inline]
    fn suspend(&self, t: MutexType) {
        let parking = match t {
            MutexType::Exclusive => &self.inner().parking_e,
            MutexType::Group => &self.inner().parking_g,
        };
        parking.push(thread::current());
        thread::park()
    }

    #[inline]
    fn wake_all(&self, t: MutexType) {
        let parking = match t {
            MutexType::Exclusive => &self.inner().parking_e,
            MutexType::Group => &self.inner().parking_g,
        };
        while let Some(thread) = parking.pop() {
            thread.unpark();
        }
    }

    #[inline]
    fn wake(&self, t: MutexType) -> bool {
        let parking = match t {
            MutexType::Exclusive => &self.inner().parking_e,
            MutexType::Group => &self.inner().parking_g,
        };
        if let Some(thread) = parking.pop() {
            thread.unpark();
            return true;
        }
        false
    }
}

impl Clone for Mutex {
    fn clone(&self) -> Self {
        self.inner().ref_count.fetch_add(1, Relaxed);
        Mutex { ptr: self.ptr }
    }
}

impl Drop for Mutex {
    fn drop(&mut self) {
        if self.inner().ref_count.fetch_sub(1, Relaxed) == 1 {
            let ptr = self.ptr as *mut InnerMutex;
            unsafe { drop(Box::from_raw(ptr)) };
        }
    }
}

impl fmt::Debug for Mutex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.inner();
        f.debug_struct("Mutex")
            .field("locked", &(inner.state.load(Relaxed) != UNLOCKED))
            .field("group", &inner.state.load(Relaxed))
            .field("lockers", &inner.locked.load(Relaxed))
            .field("ref", &inner.ref_count.load(Relaxed))
            .finish()
    }
}
