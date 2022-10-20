use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, Ordering};
use core::hint::unreachable_unchecked;
use core::{fmt, mem};

/// Synchronization primitive for creating values with one-time initializers
pub struct Once<T> {
    status: AtomicStatus,
    data: UnsafeCell<MaybeUninit<T>>
}

unsafe impl<T: Sync> Sync for Once<T> {}

impl<T: fmt::Debug> fmt::Debug for Once<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.get() {
            Some(data) => write!(f, "Once {{ data: ")
                .and_then(|_| data.fmt(f))
                .and_then(|_| write!(f, "}}")),
            None => write!(f, "Once {{ <not initialized> }}")
        }
    }
}

/// Representation of the status of a Once primitive
///
/// The inner AtomicU8 always has a value which corresponds to a valid status
#[repr(transparent)]
struct AtomicStatus(AtomicU8);

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum OnceStatus {
    /// Initializer has been run and value has been initialized
    Complete = 0,
    /// Initializer is running and value is in the process of being initialized
    Running = 1,
    /// Value has not been initialized
    Incomplete = 2,
    /// Initializer panicked while in the process of initialization
    Panicked = 3
}

impl OnceStatus {
    /// Reinterprets the u8 as a OnceStatus
    unsafe fn new(status_no: u8) -> Self {
        mem::transmute(status_no)
    }
}

impl AtomicStatus {
    #[inline(always)]
    const fn new(status: OnceStatus) -> Self {
        Self(AtomicU8::new(status as u8))
    }

    #[inline(always)]
    fn load(&self, ordering: Ordering) -> OnceStatus {
        unsafe { OnceStatus::new(self.0.load(ordering)) }
    }

    #[inline(always)]
    fn store(&self, status: OnceStatus, ordering: Ordering){
        self.0.store(status as u8, ordering);
    }

    #[inline(always)]
    fn compare_exchange(
        &self,
        old: OnceStatus,
        new: OnceStatus,
        success: Ordering,
        failure: Ordering
    ) -> Result<OnceStatus, OnceStatus> {
        match self.0.compare_exchange(old as u8, new as u8, success, failure) {
            Ok(status) => unsafe { Ok(OnceStatus::new(status)) },
            Err(status) => unsafe { Err(OnceStatus::new(status)) }
        }
    }

    #[inline(always)]
    fn get_mut(&mut self) -> &mut OnceStatus {
        unsafe { &mut *((self.0.get_mut() as *mut u8).cast::<OnceStatus>()) }
    }
}

impl<T> Once<T> {
    /// Performs an initialization routine once and only once
    pub fn call_once<F: FnOnce() -> T>(&self, f: F) -> &T {
        match self.try_call_once(|| Ok::<T, core::convert::Infallible>(f())) {
            Ok(t) => t,
            Err(void) => match void {}
        }
    }

    fn try_call_once<F: FnOnce() -> Result<T, E>, E>(&self, f: F) -> Result<&T, E> {
        let mut status = self.status.load(Ordering::Acquire);
        
        // If value is not initialized, initialize it
        if status == OnceStatus::Incomplete {
            match self.status.compare_exchange(
                OnceStatus::Incomplete,
                OnceStatus::Running,
                Ordering::Acquire,
                Ordering::Acquire
            ){
                Ok(_) => {
                    let finish = Finish { status: &self.status };
                    let val = match f(){
                        Ok(val) => val,
                        Err(err) => {
                            mem::forget(finish);
                            self.status.store(OnceStatus::Incomplete, Ordering::Release);
                            return Err(err);
                        }
                    };
                    unsafe { (*self.data.get()).as_mut_ptr().write(val) };
                    mem::forget(finish);
                    self.status.store(OnceStatus::Complete, Ordering::Release);
                    return unsafe { Ok(self.force_get()) }
                },
                Err(s) => status = s
            }
        }
        let s = match status {
            OnceStatus::Complete => unsafe { self.force_get() },
            OnceStatus::Panicked => panic!("Initializer panicked"),
            OnceStatus::Running => self.poll().unwrap(),
            OnceStatus::Incomplete => unsafe { unreachable_unchecked() }
        };
        Ok(s)
    }

    fn poll(&self) -> Option<&T> {
        loop {
            match self.status.load(Ordering::Acquire) {
                OnceStatus::Incomplete => return None,
                OnceStatus::Running => core::hint::spin_loop(),
                OnceStatus::Complete => return unsafe { Some(self.force_get()) },
                OnceStatus::Panicked => panic!("Initializer panicked")
            }
        }
    }
}

impl<T> Once<T> {
    pub const INIT: Self = Self {
        status: AtomicStatus::new(OnceStatus::Incomplete),
        data: UnsafeCell::new(MaybeUninit::uninit())
    };

    pub const fn new() -> Self {
        Self::INIT
    }

    unsafe fn force_get(&self) -> &T {
        &*(*self.data.get()).as_ptr()
    }

    pub fn get(&self) -> Option<&T> {
        match self.status.load(Ordering::Acquire) {
            OnceStatus::Complete => unsafe { Some(self.force_get()) },
            _ => None
        }
    }
}

impl<T> Drop for Once<T> {
    fn drop(&mut self) {
        if *self.status.get_mut() == OnceStatus::Complete {
            unsafe { core::ptr::drop_in_place((*self.data.get()).as_mut_ptr()) };
        }
    }
}

struct Finish<'a> {
    status: &'a AtomicStatus
}

impl<'a> Drop for Finish<'a> {
    fn drop(&mut self) {
        self.status.store(OnceStatus::Panicked, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_works() {
        let mut n = 0;
        let num = Once::new();
        num.call_once(|| n);
        n += 1;
        num.call_once(|| n);
        assert_eq!(*num.get().unwrap(), 0);
    }
}