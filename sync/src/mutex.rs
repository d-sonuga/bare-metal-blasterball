use core::cell::UnsafeCell;
use core::sync::atomic::{Ordering, AtomicBool};
use core::ops::{Deref, DerefMut, Drop};
use core::fmt;

/// A spin based synchronization primitive for mutual exclusion
pub struct Mutex<T> {
    data: UnsafeCell<T>,
    lock: AtomicBool
}

/// A guard the gives mutable access to the Mutex data
///
/// Lock is automatically released after the guard is dropped
pub struct MutexGuard<'a, T> {
    data: &'a mut T,
    lock: &'a AtomicBool
}

unsafe impl <T: Send> Sync for Mutex<T> {}
unsafe impl <T: Send> Send for Mutex<T> {}

impl<T> Mutex<T> {

    /// Creates a new Mutex
    ///
    /// # Example
    ///
    /// ```
    /// use sync::mutex::Mutex;
    ///
    /// static MUTEX: Mutex<u8> = Mutex::new(1);
    ///
    /// fn sample() {
    ///     let lock = MUTEX.lock();
    ///     // ...
    ///     drop(lock);
    /// }
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            lock: AtomicBool::new(false)
        }
    }

    /// Unwraps the underlying data, consuming the Mutex
    ///
    /// # Example
    ///
    /// ```
    /// use sync::mutex::Mutex;
    ///
    /// let lock: Mutex<u8> = Mutex::new(2);
    /// assert_eq!(2, lock.into_inner());
    /// ```
    pub fn into_inner(self) -> T {
        let Mutex { data, .. } = self;
        data.into_inner()
    }

    /// Locks the Mutex and returns a MutexGuard providing access to the underlying data
    ///
    /// # Example
    ///
    /// ```
    /// use sync::mutex::Mutex;
    ///
    /// let lock = Mutex::new(22);
    /// {
    ///     let mut data = lock.lock();
    ///     // Lock has been acquired. Data can now be accessed
    ///     *data += 23;
    ///     // Lock is dropped at the end of the scope
    /// }
    pub fn lock(&self) -> MutexGuard<T> {
        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            // Signal the processor to go into an efficient loop
            core::hint::spin_loop();
        }
        MutexGuard {
            data: unsafe { &mut *self.data.get() },
            lock: &self.lock
        }
    }

    /// Same as lock, but rather than wait for an unlock, None is simply returned
    ///
    /// # Example
    ///
    /// ```
    /// use sync::mutex::Mutex;
    ///
    /// let lock = Mutex::new(9);
    /// let guard1 = lock.try_lock();
    /// assert!(guard1.is_some());
    ///
    /// let guard2 = lock.try_lock();
    /// assert!(guard2.is_none());
    /// ```
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        if self.lock.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            Some(MutexGuard {
                data: unsafe { &mut *self.data.get() },
                lock: &self.lock
            })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the underlying data
    ///
    /// The call borrows Mutex mutably, so Rust's compile time guarantees of mutable references'
    /// mutual exclusion removes the need for locking.
    ///
    /// # Example
    ///
    /// ```
    /// use sync::mutex::Mutex;
    ///
    /// let mut lock: Mutex<u32> = Mutex::new(32);
    /// *lock.get_mut() += 2;
    /// assert_eq!(*lock.lock(), 34);
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

impl<T: fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => f.debug_struct("Mutex")
                .field("data", &*guard.data)
                .finish(),
            None => write!(f, "Mutex {{ <locked> }}")
        }
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {

    /// Releases the lock
    fn drop(&mut self) {
        self.lock.store(false, Ordering::Release);
    }
}