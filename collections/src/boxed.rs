use core::ops::{Drop, Deref, DerefMut};
use core::cmp::PartialEq;
use core::fmt;
use core::mem;
use crate::allocator::Allocator;


pub struct Box<'a, T> {
    ptr: *mut T,
    allocator: &'a dyn Allocator
}

impl<'a, T> Box<'a, T> {
    /// Creates a new heap allocated value
    pub fn new(val: T, allocator: &'a dyn Allocator) -> Box<T> {
        match unsafe { allocator.alloc(mem::size_of::<T>(), 1) } {
            Ok(ptr) => {
                let ptr = ptr as *mut T;
                unsafe { *ptr = val };
                Box {
                    ptr,
                    allocator
                }
            }
            Err(_) => panic!("No enough space on the heap")
        }
    }

    /// Consumes the box, returning the underlying pointer to the data
    pub fn into_raw(b: Box<T>) -> *mut T {
        use core::mem::ManuallyDrop;
        let b = ManuallyDrop::new(b);
        b.ptr
    }

    /// Creates a box from a raw pointer to a value already on the heap
    /// and an allocator
    ///
    /// # Safety
    ///
    /// The caller has to ensure that `ptr` is pointing to a valid area on the heap
    pub unsafe fn from_raw<'b, U>(ptr: *mut U, allocator: &'b dyn Allocator) -> Box<'b, U> {
        Box {
            ptr,
            allocator
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

impl<'a, T> Drop for Box<'a, T> {
    fn drop(&mut self) {
        if unsafe { self.allocator.dealloc(self.ptr as *mut u8, mem::size_of::<T>()).is_err() } {
            panic!("Couldn't drop the box's contents");
        }
    }
}

#[cfg(test)]
#[allow(unused_variables)]
mod tests {
    use super::*;
    use crate::allocator::{Error, Allocator};

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

    #[test]
    fn box_into_raw() {
        let b: Box<usize> = Box::new(1984, &AlwaysSuccessfulAllocator);
        let b_ptr = Box::into_raw(b);
        unsafe { assert_eq!(*b_ptr, 1984) }
    }

    #[test]
    fn box_from_raw() {
        let ptr = &100_000_000 as *const i32 as *mut i32;
        let ptr = unsafe {
            let ptr = AlwaysSuccessfulAllocator.alloc(mem::size_of::<i32>(), 1).unwrap() as *mut i32;
            *ptr = 100_000_000;
            ptr
        };
        let b = unsafe { Box::<i32>::from_raw(ptr, &AlwaysSuccessfulAllocator) };
        assert_eq!(*b, 100_000_000);
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

    #[derive(Debug)]
    struct ConditionalFailureAllocator {
        should_fail: bool
    }

    unsafe impl Allocator for ConditionalFailureAllocator {
        unsafe fn alloc(&self, size_of_type: usize, size_to_alloc: usize) -> Result<*mut u8, Error> {
            if self.should_fail {
                Err(Error::UnknownError)
            } else {
                AlwaysSuccessfulAllocator.alloc(size_of_type, size_to_alloc)
            }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, size_to_dealloc: usize)  -> Result<(), Error> {
            if self.should_fail {
                Err(Error::UnknownError)
            } else {
                AlwaysSuccessfulAllocator.dealloc(ptr, size_to_dealloc)
            }
        }
    }
}
