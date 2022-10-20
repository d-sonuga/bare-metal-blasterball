//! A contiguous growable array with heap-allocated contents

use core::ops::{Drop, Index, IndexMut};
use core::cmp::PartialEq;
use core::iter::Iterator;
use core::mem;
use core::fmt;
use crate::allocator::Allocator;

/// A first in first out structure
pub struct Queue<'a, T: Clone> {
    /// This always holds the number of `T` items in the queue
    len: usize,
    /// This always holds the number of `T` items the queue is capable of holding
    capacity: usize,
    /// This always holds the pointer to the start of the memory allocated for the queue
    start_ptr: *mut T,
    /// This always holds the pointer to the location where the front of the queue is
    front_ptr: *mut T,
    /// This always holds the pointer to the location where the next `T` item should be stored in
    back_ptr: *mut T,
    /// The allocator used to allocate and deallocate memory for the queue
    allocator: &'a dyn Allocator
}

impl<'a, T: Clone> Queue<'a, T> {

    /// Creates a queue with the stated capacity
    ///
    /// Running time depends on the speed of the allocator.
    ///
    /// # Panics
    ///
    /// If there is no enough space on the heap
    pub fn with_capacity(capacity: usize, allocator: &dyn Allocator) -> Queue<T> {
        match unsafe { allocator.alloc(mem::size_of::<T>(), capacity) } {
            Ok(ptr) => Queue {
                len: 0,
                capacity,
                start_ptr: ptr as *mut T,
                front_ptr: ptr as *mut T,
                back_ptr: ptr as *mut T,
                allocator
            },
            Err(_) => panic!("No enough space on the heap")
        }
    }
    
    /// Places an item at the back of the queue
    ///
    /// # Complexity
    /// Takes O(1) amortized time
    ///
    /// On a regular day, while there is still enough capacity, this will take O(1) time.
    /// But when the capacity is filled, all the items are copied into a newly allocated location
    /// with 2x the size, which will take O(n) time, where n == the number of items in the queue
    ///
    /// # Panics
    /// This function panics in the event where more memory is needed for the queue
    /// but the allocator fails to provide it
    pub fn enqueue(&mut self, item: T) {
        if self.len >= self.capacity {
            let new_size = self.capacity * 2;
            let old_size = self.capacity;
            let old_start_ptr = self.start_ptr as *mut u8;
            let alloc_result = unsafe { self.allocator.alloc(mem::size_of::<T>(), new_size) };
            let len = self.len;
            if alloc_result.is_err() {
                panic!("No enough space on the heap.");
            }
            let new_start_ptr = alloc_result.unwrap() as *mut T;
            for i in 0..self.len as isize {
                unsafe {
                    new_start_ptr.offset(i).write(self.dequeue().unwrap());
                }
            }
            unsafe { self.allocator.dealloc(old_start_ptr, old_size * mem::size_of::<T>()).unwrap() };
            self.len = len;
            self.capacity = new_size;
            self.start_ptr = new_start_ptr as *mut T;
            self.front_ptr = new_start_ptr as *mut T;
            unsafe { self.back_ptr = self.start_ptr.offset(self.len as isize) };
        }
        unsafe {
            self.back_ptr.write(item);
            // Items in the queue's chunk of memory can only be in indexes 0..=capacity - 1
            let after_last_pos_ptr = self.start_ptr.offset(self.capacity as isize);
            let new_back_ptr = self.back_ptr.offset(1);
            // back_ptr has crossed the 0..capacity - 1 boundary
            // Wrap around
            if new_back_ptr == after_last_pos_ptr {
                self.back_ptr = self.start_ptr;
            } else {
                self.back_ptr = new_back_ptr;
            }
        }
        self.len += 1;
    }

    /// Removes and returns the item at the front of the queue, if there is any
    ///
    /// # Complexity
    /// Takes O(1) time, since it's just removing an item and updating pointers
    pub fn dequeue(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            unsafe {
                let val = self.front_ptr.read();
                self.front_ptr = self.front_ptr.offset(1);
                // Items in the queue's chunk of memory can only be in indexes 0..=capacity - 1
                let after_last_pos_ptr = self.start_ptr.offset(self.capacity as isize);
                // front_ptr has crossed the 0..capacity - 1 boundary
                // Wrap around
                if self.front_ptr == after_last_pos_ptr {
                    self.front_ptr = self.start_ptr;
                }
                self.len -= 1;
                Some(val)
            }
        }
    }

    /// Returns the number of items in the queue
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the capacity of the queue
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<'a, T: Clone> Drop for Queue<'a, T> {
    fn drop(&mut self) {
        use core::ptr;
        unsafe {
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.start_ptr, self.len));
            self.allocator.dealloc(self.start_ptr as *mut u8, mem::size_of::<T>() * self.capacity).unwrap()
        };
    }
}

#[macro_export]
macro_rules! queue {
    (item_type => $T:ty, capacity => $e:expr, $allocator:expr) => {
        {
            let queue: Queue<$T> = Queue::with_capacity($e, $allocator);
            queue
        }
    };
    (item_type => $T:ty, capacity => $e:expr) => {
        {
            use $crate::allocator::get_allocator;
            let allocator = get_allocator();
            queue!(item_type => $T, capacity => $e, allocator)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::{Error, Allocator};

    #[test]
    fn test_create() {
        let mut queue: Queue<u8> = Queue::with_capacity(100, &AlwaysSuccessfulAllocator);
        assert_eq!(queue.capacity(), 100)
    }

    #[test]
    fn test_enqueue_dequeue1() {
        let mut queue: Queue<u128> = Queue::with_capacity(100, &AlwaysSuccessfulAllocator);
        queue.enqueue(32);
        queue.enqueue(100);
        assert_eq!(queue.len(), 2);

        let first = queue.dequeue();
        let second = queue.dequeue();
        assert_eq!(first, Some(32));
        assert_eq!(second, Some(100));
        assert_eq!(queue.len(), 0);
    }


    #[test]
    fn test_enqueue_dequeue_need_more_space() {
        let mut queue: ManuallyDrop<Queue<u8>> = ManuallyDrop::new(Queue::with_capacity(3, &AlwaysSuccessfulAllocator));
        queue.enqueue(43);
        queue.enqueue(133);
        queue.enqueue(54);
        assert_eq!(queue.capacity(), 3);
        assert_eq!(queue.len(), 3);
        queue.enqueue(255);
        assert!(queue.capacity() > 3);
        assert_eq!(queue.len(), 4);
        assert_eq!(queue.dequeue(), Some(43));
        assert_eq!(queue.dequeue(), Some(133));
        assert_eq!(queue.dequeue(), Some(54));
        assert_eq!(queue.dequeue(), Some(255));
    }

    #[test]
    fn test_enqueue_dequeue2() {
        let mut queue: Queue<u8> = Queue::with_capacity(5, &AlwaysSuccessfulAllocator);
        queue.enqueue(32);
        queue.enqueue(230);
        queue.enqueue(254);
        let first = queue.dequeue();
        let second = queue.dequeue();
        queue.enqueue(43);
        queue.enqueue(129);
        queue.enqueue(233);
        queue.enqueue(12);
        let third = queue.dequeue();
        let fourth = queue.dequeue();
        let fifth = queue.dequeue();
        let sixth = queue.dequeue();
        let seventh = queue.dequeue();
        assert_eq!(first, Some(32));
        assert_eq!(second, Some(230));
        assert_eq!(third, Some(254));
        assert_eq!(fourth, Some(43));
        assert_eq!(fifth, Some(129));
        assert_eq!(sixth, Some(233));
        assert_eq!(seventh, Some(12));
        assert_eq!(queue.dequeue(), None);
    }

    #[test]
    fn test_dequeue_empty_queue() {
        let mut queue: Queue<u8> = Queue::with_capacity(3, &AlwaysSuccessfulAllocator);
        assert_eq!(queue.dequeue(), None);
    }

    #[test]
    #[should_panic]
    fn test_create_queue_alloc_fail() {
        let cond_failure_allocator = ConditionalFailureAllocator { should_fail: true };
        let queue: Queue<u8> = Queue::with_capacity(1, &cond_failure_allocator);
    }

    macro_rules! mutate_cond_fail_alloc {
        ($cond_fail_allocator:ident, should_fail => $e:expr) => {
            (*(&$cond_fail_allocator as *const _ as *mut ConditionalFailureAllocator)).should_fail = $e;
        }
    }

    #[test]
    #[should_panic]
    fn test_out_of_space_on_enqueue() {
        use core::mem::ManuallyDrop;
        let mut cond_failure_allocator = ConditionalFailureAllocator { should_fail: false };
        // Using ManuallyDrop to avoid double panics because the dealloc function is called in drop
        let mut queue: ManuallyDrop<Queue<u32>> = ManuallyDrop::new(Queue::with_capacity(1, &cond_failure_allocator));
        queue.enqueue(3);
        assert_eq!(unsafe { queue.front_ptr.read() }, 3);
        unsafe { mutate_cond_fail_alloc!(cond_failure_allocator, should_fail => true) };
        queue.enqueue(32);
    }

    #[test]
    #[should_panic]
    fn test_failure_on_dealloc() {
        let mut cond_failure_allocator = ConditionalFailureAllocator { should_fail: false };
        let mut v: Queue<bool> = Queue::with_capacity(3, &cond_failure_allocator);
        unsafe { mutate_cond_fail_alloc!(cond_failure_allocator, should_fail => true) };
        // dealloc is called on drop
    }

    #[test]
    fn test_queue_of_structs() {
        #[derive(Clone)]
        struct SomeValues {
            x: i32,
            y: usize,
            z: i128
        };
        let mut queue = Queue::with_capacity(2, &AlwaysSuccessfulAllocator);
        queue.enqueue(SomeValues { x: 32, y: 54_444, z: 889_987_233_554 });
        queue.enqueue(SomeValues { x: 890, y: 5_343, z: 335_232 });
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_macro() {
        let allocator = &AlwaysSuccessfulAllocator;
        let mut queue = crate::queue!(item_type => u8, capacity => 10, allocator);
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.capacity(), 10);
        queue.enqueue(56);
        assert_eq!(queue.dequeue(), Some(56));
    }

    struct AlwaysSuccessfulAllocator;

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