use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::thread;
use std::time::{Duration, Instant};

type State = u8;

const UNLOCKED: State = 0;
const LOCKED: State = 1; // locked, no other threads waiting
const CONTENDED: State = 2; // locked, and other threads waiting (contended)

pub struct Mutex {
    state: AtomicU8,
}

impl Mutex {
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(UNLOCKED),
        }
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        self.state
            .compare_exchange(UNLOCKED, LOCKED, Acquire, Relaxed)
            .is_ok()
    }

    #[inline]
    pub fn lock(&self) {
        if self
            .state
            .compare_exchange(UNLOCKED, LOCKED, Acquire, Relaxed)
            .is_err()
        {
            self.lock_contended();
        }
    }

    #[cold]
    fn lock_contended(&self) {
        // Spin first to speed things up if the lock is released quickly.
        let mut state = self.spin(100);

        // If it's unlocked now, attempt to take the lock
        // without marking it as contended.
        if state == UNLOCKED {
            match self
                .state
                .compare_exchange(UNLOCKED, LOCKED, Acquire, Relaxed)
            {
                Ok(_) => return, // Locked!
                Err(s) => state = s,
            }
        }

        loop {
            // Put the lock in contended state.
            // We avoid an unnecessary write if it as already set to CONTENDED,
            // to be friendlier for the caches.
            if state != CONTENDED && self.state.swap(CONTENDED, Acquire) == UNLOCKED {
                // We changed it from UNLOCKED to CONTENDED, so we just successfully locked it.
                return;
            }

            // Wait for the futex to change state, assuming it is still CONTENDED.
            us_wait(&self.state, CONTENDED, None);

            // Spin again after waking up.
            state = self.spin(100);
        }
    }

    fn spin(&self, mut spin: i32) -> State {
        loop {
            // We only use `load` (and not `swap` or `compare_exchange`)
            // while spinning, to be easier on the caches.
            let state = self.state.load(Relaxed);

            // We stop spinning when the mutex is UNLOCKED,
            // but also when it's CONTENDED.
            if state != LOCKED || spin == 0 {
                return state;
            }

            std::hint::spin_loop();
            spin -= 1;
        }
    }

    #[inline]
    pub fn unlock(&self) {
        if self.state.swap(UNLOCKED, Release) == CONTENDED {
            // We only wake up one thread. When that thread locks the mutex, it
            // will mark the mutex as CONTENDED (see lock_contended above),
            // which makes sure that any other waiting threads will also be
            // woken up eventually.
            self.wake();
        }
    }

    #[cold]
    fn wake(&self) {
        //...
    }
}

fn us_wait(state: &AtomicU8, expected: u8, timeout: Option<Duration>) {
    let mut backoff = 1;

    if let Some(max_dur) = timeout {
        let start = Instant::now();

        while state.load(Acquire) == expected {
            if start.elapsed() >= max_dur {
                break;
            }
            backoff = cpu_relax(backoff);
        }
    } else {
        while state.load(Acquire) == expected {
            backoff = cpu_relax(backoff);
        }
    }
}

#[inline(always)]
fn cpu_relax(backoff: i32) -> i32 {
    // Yield processor or backoff
    if backoff <= 64 {
        std::hint::spin_loop();
    } else if backoff <= 512 {
        thread::yield_now();
    } else {
        thread::sleep(Duration::from_micros(backoff as u64));
    }

    (backoff * 2).min(10_000)
}
