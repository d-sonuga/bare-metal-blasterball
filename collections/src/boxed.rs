use core::ops::{Drop, Deref, DerefMut};
use core::cmp::PartialEq;
use core::fmt;
use core::mem;
use crate::allocator::{Allocator, get_allocator};


pub struct Box<'a, T: ?Sized> {
    ptr: *mut T,
    allocator: &'a dyn Allocator
}

impl<'a, T> Box<'a, T> {
    /// Creates a new heap allocated value
    pub fn new(val: T, allocator: &dyn Allocator) -> Box<T> {
        match unsafe { allocator.alloc(mem::size_of::<T>(), 1) } {
            Ok(ptr) => {
                let mut ptr = ptr as *mut T;
                unsafe { *ptr = val };
                Box {
                    ptr,
                    allocator
                }
            }
            Err(_) => panic!("No enough space on the heap")
        }
    }
}

impl<'a, T> Deref for Box<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<'a, T> DerefMut for Box<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

impl<'a, 'b, T: PartialEq> PartialEq<Box<'b, T>> for Box<'a, T> {
    fn eq(&self, other: &Box<T>) -> bool {
        unsafe { *self.ptr == *other.ptr }
    }
}

impl<'a, T: PartialEq> PartialEq<T> for Box<'a, T> {
    fn eq(&self, other: &T) -> bool {
        unsafe { *self.ptr == *other }
    }
}

impl<'a, T: fmt::Debug> fmt::Debug for Box<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Box")
            .field("val", unsafe { &*self.ptr })
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::{Error, Allocator};

    macro_rules! mutate_cond_fail_alloc {
        ($cond_fail_allocator:ident, should_fail => $e:expr) => {
            (*(&$cond_fail_allocator as *const _ as *mut ConditionalFailureAllocator)).should_fail = $e;
        }
    }

    #[test]
    fn test_box_create() {
        let b: Box<i32> = Box::new(32, &AlwaysSuccessfulAllocator);
        assert_eq!(b, 32);
    }

    #[test]
    fn test_cmp() {
        let b1: Box<i32> = Box::new(43, &AlwaysSuccessfulAllocator);
        let b2: Box<i32> = Box::new(43, &AlwaysSuccessfulAllocator);
        assert_eq!(b1, b2);
    }

    #[test]
    fn test_mutate() {
        let mut b: Box<i32> = Box::new(45, &AlwaysSuccessfulAllocator);
        *b = 999_999;
        assert_eq!(b, 999_999);
    }

    #[test]
    #[should_panic]
    fn box_out_of_space() {
        let allocator = ConditionalFailureAllocator { should_fail: true };
        let b: Box<usize> = Box::new(32, &allocator);

    }

    struct AlwaysSuccessfulAllocator;

    use std::vec::Vec as StdVec;
    use core::mem::ManuallyDrop;

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

    #[derive(Debug)]
    struct ConditionalFailureAllocator {
        should_fail: bool
    }

    unsafe impl Allocator for ConditionalFailureAllocator {
        unsafe fn alloc(&self, size_of_type: usize, size_to_alloc: usize) -> Result<*mut u8, Error> {
            use crate::allocator::Error;
            if self.should_fail {
                Err(Error::UnknownError)
            } else {
                AlwaysSuccessfulAllocator.alloc(size_of_type, size_to_alloc)
            }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, size_to_dealloc: usize)  -> Result<(), Error> {
            use crate::allocator::Error;
            if self.should_fail {
                Err(Error::UnknownError)
            } else {
                AlwaysSuccessfulAllocator.dealloc(ptr, size_to_dealloc)
            }
        }
    }
}
