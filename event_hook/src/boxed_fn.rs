//! A structure for creating polymorphic functions for event handlers
//!
//! Necessary because we can't use trait objects which require special support
//! from the compiler through the alloc library, which we are not using

use core::ops::{Fn, FnMut, FnOnce};
use core::ptr::NonNull;
use core::marker::PhantomData;
use core::fmt;
use collections::allocator::Allocator;
use collections::boxed::Box;
use crate::Event;

/// The polymorphic representation of a base function which can stand in
/// for any event handler function
#[repr(C)]
struct BaseFn {
    call_boxed: fn(*const BoxedFn, Event) -> (),
    drop: fn(*const BoxedFn) -> ()
}

/// A stand in for `Box<dyn FnMut>`
///
/// Main idea gotten from <https://adventures.michaelfbryan.com/posts/ffi-safe-polymorphism-in-rust/>
#[derive(Clone)]
pub struct BoxedFn<'a>(NonNull<BaseFn>, &'a dyn Allocator);

impl<'a> BoxedFn<'a> {
    /// Creates a new BoxedFn from the given function and returns the
    /// polymorphic BaseFn
    pub fn new<F>(func: F, allocator: &'a dyn Allocator) -> Self where F: FnMut(Event) {
        let concrete_repr = Repr {
            base: BaseFn { call_boxed: call_boxed::<F>, drop: drop::<F> },
            func
        };
        let concrete_repr_ptr: *mut Repr<F> = Box::<Repr<F>>::into_raw(Box::new(concrete_repr, allocator));
        let polymorphic_ptr: *mut BaseFn = concrete_repr_ptr as *mut BaseFn;
        BoxedFn(NonNull::new(polymorphic_ptr).unwrap(), allocator)
    }
}

/// Calls the concrete function wrapped by the BoxedFunction
fn call_boxed<F>(boxed_fn_ptr: *const BoxedFn, event: Event) where F: FnMut(Event) {
    unsafe {
        let concrete_repr_ptr = (*boxed_fn_ptr).0.as_ptr() as *mut BaseFn as *mut Repr<F>;
        ((*concrete_repr_ptr).func)(event) 
    }
}

/// Drops the boxed function
fn drop<F>(boxed_fn_ptr: *const BoxedFn) where F: FnMut(Event) {
    unsafe {
        let base_fn_ptr = (*boxed_fn_ptr).0.as_ptr();
        let concrete_ptr: *mut Repr<F> = base_fn_ptr as *mut Repr<F>;
        let allocator = (*boxed_fn_ptr).1;
        Box::<Repr<F>>::from_raw(concrete_ptr, allocator);
        // Box is dropped at the end of the scope
    }
}

impl<'a> Drop for BoxedFn<'a> {
    fn drop(&mut self) {
        let base_fn_ptr = self.0.as_ptr();
        unsafe { ((*base_fn_ptr).drop)(self as *const BoxedFn) };
    }
}

#[repr(C)]
struct Repr<F: FnMut(Event)> {
    base: BaseFn,
    func: F
}

impl<'a> Fn<(Event,)> for BoxedFn<'a> {
    extern "rust-call" fn call(&self, args: (Event,)) -> Self::Output {
        let base_fn_ptr = self.0.as_ptr();
        unsafe { ((*base_fn_ptr).call_boxed)(self as *const BoxedFn, args.0) }
    }
}

impl<'a> FnMut<(Event,)> for BoxedFn<'a> {
    extern "rust-call" fn call_mut(&mut self, args: (Event,)) -> Self::Output {
        self.call(args)
    }
}

impl<'a> FnOnce<(Event,)> for BoxedFn<'a> {
    type Output = ();
    extern "rust-call" fn call_once(self, args: (Event,)) -> Self::Output {
        self.call(args)
    }
}

impl fmt::Debug for BoxedFn<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("BoxedFn")
            .field(&self.0)
            .finish()
    }
}

#[macro_export]
macro_rules! box_fn {
    ($f:expr) => {
        {
            use collections::allocator::get_allocator;
            $crate::box_fn!($f, get_allocator())
        }
    };
    ($f:expr, $alloc:expr) => {
        {
            use $crate::boxed_fn::BoxedFn;
            BoxedFn::new($f, $alloc)
        }
    };
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use collections::allocator::{Allocator, Error};
    use collections::vec;

    #[test]
    fn test_fn_call() {
        let mut was_called = false;
        let f: _ = BoxedFn::new(|_| {
            was_called = true;
        }, &AlwaysSuccessfulAllocator);
        assert!(!was_called);
        f(Event::Timer);
        assert!(was_called);
    }

    #[test]
    fn test_box_fn_macro() {
        let allocator = &AlwaysSuccessfulAllocator;
        let mut was_called = false;
        let f = box_fn!(|_| {
            was_called = true;
        }, allocator);
        assert!(!was_called);
        f(Event::Timer);
        assert!(was_called);
    }

    #[test]
    fn test_vec_of_boxed_fn() {
        let mut no_of_fns_called = 0;
        let allocator = &AlwaysSuccessfulAllocator;
        let v: collections::vec::Vec<BoxedFn> = collections::vec![
            box_fn!(|_| no_of_fns_called += 1, allocator),
            box_fn!(|_| no_of_fns_called += 1, allocator),
            box_fn!(|_| no_of_fns_called += 1, allocator);
            allocator
        ];
        v.iter().for_each(|f| f(Event::Timer));
        assert_eq!(no_of_fns_called, 3);
    }

    #[test]
    fn test_boxed_fn_drop() {
        let mut x = 1;
        {
            let allocator = &AlwaysSuccessfulAllocator;
            box_fn!(|_| x += 1, allocator);
        }
    }

    #[test]
    fn test_boxed_fn_remove_from_vec() {
        let mut x = 1;
        let allocator = &AlwaysSuccessfulAllocator;
        {
            let mut v: collections::vec::Vec<BoxedFn> = collections::vec![
                box_fn!(|_| x += 1, allocator),
                box_fn!(|_| x += 1, allocator),
                box_fn!(|_| x += 1, allocator);
                allocator
            ];
            v.remove(1);
            assert_eq!(v.len(), 2);
        }
    }

    pub struct AlwaysSuccessfulAllocator;

    use std::vec::Vec as StdVec;
    use core::mem::ManuallyDrop;
    use core::mem;

    unsafe impl Allocator for AlwaysSuccessfulAllocator {
        unsafe fn alloc(&self, size_of_type: usize, size_to_alloc: usize) -> Result<*mut u8, Error> {
            let mut v: ManuallyDrop<StdVec<u8>> = ManuallyDrop::new(StdVec::with_capacity(size_of_type * size_to_alloc));
            Ok(v.as_mut_ptr() as *mut u8)
        }

        unsafe fn dealloc(&self, ptr: *mut u8, size_to_dealloc: usize)  -> Result<(), Error> {
            let v: StdVec<u8> = StdVec::from_raw_parts(ptr, size_to_dealloc, size_to_dealloc);
            mem::drop(v);
            Ok(())
        }
    }
}

