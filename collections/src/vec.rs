//! A contiguous growable array with heap-allocated contents

use core::ops::{Drop, Index, IndexMut};
use core::cmp::PartialEq;
use core::iter::Iterator;
use core::mem;
use core::fmt;
use crate::allocator::Allocator;

pub struct Vec<'a, T: Clone> {
    len: usize,
    capacity: usize,
    start_ptr: *mut T,
    allocator: &'a dyn Allocator
}

impl<'a, T: Clone> Vec<'a, T> {

    /// Creates a vector with the stated capacity
    ///
    /// Running time depends on the speed of the allocator.
    ///
    /// # Panics
    ///
    /// If there is no enough space on the heap
    pub fn with_capacity(capacity: usize, allocator: &dyn Allocator) -> Vec<T> {
        match unsafe { allocator.alloc(mem::size_of::<T>(), capacity) } {
            Ok(ptr) => Vec {
                len: 0,
                capacity,
                start_ptr: ptr as *mut T,
                allocator
            },
            Err(_) => panic!("No enough space on the heap")
        }
    }

    /// Appends an item to the end of the vector.
    /// If the vector is full, it will allocate another vector with double the capacity
    /// and copy contents over to the new vector.
    ///
    /// Running time is O(1). O(n) in the case where all contents have to be copied over into
    /// new vector
    ///
    /// # Panics
    ///
    /// If there is no enough space on the heap
    pub fn push(&mut self, item: T) {
        if self.len >= self.capacity {
            let new_size = self.capacity * 2;
            let old_size = self.capacity;
            let old_start_ptr = self.start_ptr as *mut u8;
            let alloc_result = unsafe { self.allocator.alloc(mem::size_of::<T>(), new_size) };
            if alloc_result.is_err() {
                panic!("No enough space on the heap.");
            }
            let new_start_ptr = alloc_result.unwrap() as *mut T;
            for i in 0..self.len {
                unsafe {
                    let val = self.start_ptr.offset(i as isize).read();
                    new_start_ptr.offset(i as isize).write(val);
                }
            }
            unsafe { self.allocator.dealloc(old_start_ptr, old_size * mem::size_of::<T>()).unwrap() };
            self.capacity = new_size;
            self.start_ptr = new_start_ptr as *mut T;   
        }
        unsafe { self.start_ptr.offset(self.len as isize).write(item) };
        self.len += 1;
    }

    /// Removes an item from the end of the vector and returns it
    ///
    /// Running time is O(1)
    ///
    /// # Panics
    ///
    /// If the vector is empty
    pub fn pop(&mut self) -> T {
        if self.len == 0 {
            panic!("No items to pop");
        }
        self.len -= 1;
        unsafe { self.start_ptr.offset(self.len as isize).read() }
    }

    /// Does the same as pop, but returns a None if the vector is empty
    pub fn try_pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            Some(self.pop())
        }
    }

    /// Removes the item at index idx and returns it
    ///
    /// Running time is O(n) because all items after the item with index idx
    /// must be shifted upwards
    ///
    /// # Panics
    ///
    /// When idx is an invalid index
    pub fn remove(&mut self, idx: usize) -> T {
        if idx >= self.len {
            panic!("Invalid index");
        }
        let value = unsafe { self.start_ptr.offset(idx as isize).read() };
        for i in idx + 1..self.len {
            let i = i as isize;
            unsafe {
                let val = self.start_ptr.offset(i).read();
                self.start_ptr.offset(i - 1).write(val);
            }
        }
        self.len -= 1;
        value
    }

    /// Returns the number of items in the vector
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the capacity of the vector
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the pointer to the vector data
    pub fn as_ptr(&self) -> *const T {
        self.start_ptr
    }

    /// Creates a new iterator over the references of the vector
    pub fn iter(&self) -> core::slice::Iter<T> {
        unsafe { core::slice::from_raw_parts(self.start_ptr as *const T, self.len) }
            .iter()
    }

    /// Creates a new iterator over mutable references of the vector
    pub fn iter_mut(&mut self) -> core::slice::IterMut<T> {
        unsafe { core::slice::from_raw_parts_mut(self.start_ptr, self.len) }
            .iter_mut()
    }
}

impl<'a, T: Clone> Drop for Vec<'a, T> {
    fn drop(&mut self) {
        use core::ptr;
        unsafe {
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.start_ptr, self.len));
            self.allocator.dealloc(self.start_ptr as *mut u8, self.capacity * mem::size_of::<T>()).unwrap()
        };
    }
}

impl<'a, T: Clone> Index<usize> for Vec<'a, T> {
    type Output = T;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < self.len);
        unsafe { &*self.start_ptr.offset(idx as isize) }
    }
}

impl<'a, T: Clone> IndexMut<usize> for Vec<'a, T> {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        assert!(idx < self.len);
        unsafe { &mut *self.start_ptr.offset(idx as isize) }
    }
}

impl<'a, 'b, T: PartialEq + Clone> PartialEq<Vec<'b, T>> for Vec<'a, T> {
    fn eq(&self, other: &Vec<'b, T>) -> bool {
        for (val1, val2) in self.iter().zip(other.iter()) {
            if val1 != val2 {
                return false;
            }
        }
        true
    }
}

impl<'a, T: Clone + fmt::Debug> fmt::Debug for Vec<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Vec [ ")?;
        self.iter()
            .enumerate()
            .for_each(|(i, val)| {
                f.write_fmt(format_args!("{:?}", val)).unwrap();
                if i != self.len() {
                    f.write_str(", ").unwrap();
                }
            });
        f.write_str("]")?;
        Ok(())
    }
}

impl<'a, T: Clone> Clone for Vec<'a, T> {
    fn clone(&self) -> Self {
        let mut new_vec = Vec::with_capacity(self.capacity, self.allocator);
        self
            .iter()
            .for_each(|val| new_vec.push(val.clone()));
        new_vec
    }
}

#[macro_export]
macro_rules! vec {
    // Remember, over-optimization is the root of all evil
    ($($e:expr),+ ; $alloc:ident) => {
        {
            let mut v = $crate::vec::Vec::with_capacity(1, $alloc);
            $({
                v.push($e);
            })+
            v
        }
    };
    ($e:expr ; $n:expr ; $alloc:ident) => {
        {
            let mut v = $crate::vec::Vec::with_capacity($n, $alloc);
            for _ in 0..$n {
                v.push($e);
            }
            v
        }
    };
    ($($e:expr),+ ; &$alloc:ident) => {
        {
            let allocator = &$alloc;
            $crate::vec![$($e),+ ; allocator]
        }
    };
    ($e:expr ; $n:expr ; &$alloc:ident) => {
        {
            let allocator = &$alloc;
            $crate::vec![$e ; $n ; allocator]
        }
    };
    ($e:expr ; $n:expr) => {
        {
            use $crate::allocator::get_allocator;
            let allocator = get_allocator();
            $crate::vec![$e ; $n ; allocator]
        }
    };
    ($($e:expr),+) => {
        {
            use $crate::allocator::get_allocator;
            let allocator = get_allocator();
            vec![$($e),+ ; allocator]
        }
    };
    (item_type => $T:ty, capacity => $e:expr) => {
        {
            use $crate::allocator::get_allocator;
            let allocator = get_allocator();
            Vec::<$T>::with_capacity($e, allocator)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::Error;

    #[test]
    fn test_create() {
        let mut v: Vec<u8> = Vec::with_capacity(100, &AlwaysSuccessfulAllocator);
        assert_eq!(v.capacity(), 100);
    }

    #[test]
    fn test_push() {
        let mut v = Vec::with_capacity(3, &AlwaysSuccessfulAllocator);
        v.push(2);
        v.push(4);
        assert_eq!(v[0], 2);
        assert_eq!(v[1], 4);
    }

    #[test]
    fn test_pop() {
        let mut v = Vec::with_capacity(3, &AlwaysSuccessfulAllocator);
        v.push(2);
        v.push(100);
        assert_eq!(v.pop(), 100);
        assert_eq!(v.pop(), 2);
    }

    #[test]
    fn test_remove() {
        let mut v = Vec::with_capacity(3, &AlwaysSuccessfulAllocator);
        v.push(2);
        v.push(3);
        v.push(122);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1], 3);

        assert_eq!(v.remove(1), 3);

        assert_eq!(v[0], 2);
        assert_eq!(v[1], 122);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_index() {
        let mut v = Vec::with_capacity(5, &AlwaysSuccessfulAllocator);
        v.push(2);
        assert_eq!(v[0], 2);
        v.push(3);
        assert_eq!(v[1], 3);
        v[0] = 100_000;
        assert_eq!(v[0], 100_000);
    }

    #[test]
    fn test_need_more_space() {
        let mut v = Vec::with_capacity(1, &AlwaysSuccessfulAllocator);
        v.push(2);
        v.push(32);
        v.push(23);
        v.push(1);
        v.push(900);
        assert_eq!(v.len(), 5);
    }

    #[test]
    fn test_macro_1() {
        let mut v = crate::vec![3, 4, 54_444, 23, 2; &AlwaysSuccessfulAllocator];
        assert_eq!(v.len(), 5);
        assert_eq!(v[0], 3);
        assert_eq!(v[1], 4);
        assert_eq!(v[2], 54_444);
        assert_eq!(v[3], 23);
        assert_eq!(v[4], 2);
    }

    #[test]
    fn test_macro_2() {
        let mut v = crate::vec![0u8; 5; &AlwaysSuccessfulAllocator];
        assert_eq!(v.len(), 5);
        for i in 0..v.len() {
            assert_eq!(v[i], 0);
        }
    }

    #[test]
    fn test_iter() {
        let v = crate::vec![2, 4; &AlwaysSuccessfulAllocator];
        let mut v_iter = v.iter();
        assert_eq!(v_iter.next(), Some(&2));
        assert_eq!(v_iter.next(), Some(&4));
        assert_eq!(v_iter.next(), None);
    }

    #[test]
    fn test_iter_mut() {
        let mut v = crate::vec![4, 9; &AlwaysSuccessfulAllocator];
        {
            let mut v_iter_mut = v.iter_mut();
            let first_item = v_iter_mut.next();
            assert_eq!(first_item, Some(&mut 4));
            let first_item = first_item.unwrap();
            *first_item = 999_999_999;
        }
        assert_eq!(v[0], 999_999_999);
    }

    #[test]
    #[should_panic]
    fn test_create_vec_alloc_fail() {
        let cond_failure_allocator = ConditionalFailureAllocator { should_fail: true };
        let v: Vec<u8> = Vec::with_capacity(1, &cond_failure_allocator);        
    }

    macro_rules! mutate_cond_fail_alloc {
        ($cond_fail_allocator:ident, should_fail => $e:expr) => {
            (*(&$cond_fail_allocator as *const _ as *mut ConditionalFailureAllocator)).should_fail = $e;
        }
    }

    #[test]
    #[should_panic]
    fn test_out_of_space_on_push() {
        use core::mem::ManuallyDrop;
        let mut cond_failure_allocator = ConditionalFailureAllocator { should_fail: false };
        // Using ManuallyDrop to avoid double panics because the dealloc function is called in drop
        let mut v: ManuallyDrop<Vec<u32>> = ManuallyDrop::new(Vec::with_capacity(1, &cond_failure_allocator));
        v.push(3);
        assert_eq!(v[0], 3);
        unsafe { mutate_cond_fail_alloc!(cond_failure_allocator, should_fail => true) };
        v.push(3);
    }

    #[test]
    #[should_panic]
    fn test_failure_on_dealloc() {
        let mut cond_failure_allocator = ConditionalFailureAllocator { should_fail: false };
        let mut v: Vec<bool> = Vec::with_capacity(3, &cond_failure_allocator);
        unsafe { mutate_cond_fail_alloc!(cond_failure_allocator, should_fail => true) };
        // dealloc is called on drop
    }

    #[test]
    fn test_vec_of_structs() {
        #[derive(Clone)]
        struct SomeValues {
            x: i32,
            y: usize,
            z: i128
        };
        let mut v = Vec::with_capacity(2, &AlwaysSuccessfulAllocator);
        v.push(SomeValues { x: 32, y: 54_444, z: 889_987_233_554 });
        v.push(SomeValues { x: 890, y: 5_343, z: 335_232 });
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn vec_clone() {
        let v = crate::vec![4, 5, 87777; &AlwaysSuccessfulAllocator];
        let other_v = v.clone();
        assert_eq!(v, other_v);
        assert_eq!(v.len(), other_v.len());
        assert_eq!(v.capacity(), other_v.capacity());
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
