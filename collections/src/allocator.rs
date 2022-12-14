use core::mem;
use sync::mutex::Mutex;
use machine::memory::{MemChunk, Addr};
use lazy_static::lazy_static;

/// The trait for structs that should be used as heap allocators
/// for the collections
pub unsafe trait Allocator {
    unsafe fn alloc(&self, size_of_type: usize, size_to_alloc: usize) -> Result<*mut u8, Error>;

    unsafe fn dealloc(&self, ptr: *mut u8, size_to_dealloc: usize)  -> Result<(), Error>;
}

#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub enum Error {
    UnknownError,
    /// Thrown when alloc is called and no free memory was found
    AllocationError
}
lazy_static! {
    static ref ALLOCATOR: Mutex<LinkedListAllocator> = Mutex::new(
        LinkedListAllocator {
            head: ListNode {
                size: 0,
                next: None
            }
        }
    );
}

/// Retrieves a reference to the allocator
pub fn get_allocator() -> &'static Mutex<LinkedListAllocator> {
    &ALLOCATOR
}

/// Creates a new LinkedListAllocator, assuming that all memory
/// in heap_mem's range is free
pub fn init(heap_mem: MemChunk) {
    unsafe {
        ALLOCATOR.lock().add_free_region(heap_mem);
    }
}

/// Representation of a free region of memory
#[derive(PartialEq, Eq, Debug)]
struct ListNode {
    /// The size of the free region of memory
    size: u64,
    /// The next free region
    next: Option<*mut ListNode>
}

unsafe impl Send for ListNode {}

impl ListNode { 
    fn start_addr(&self) -> Addr {
        Addr::new(self as *const _ as u64)
    }

    fn end_addr(&self) -> Addr {
        self.start_addr() + self.size
    }
}

/// A first fit allocator for managing heap memory.
/// The free regions are kept track of with a linked list whose nodes are the
/// free regions themselves.
/// That is, the free regions hold their own information
///
/// Note: This allocator was hacked together with raw pointers, because I didn't like the
/// stress references were giving me
#[derive(Debug)]
pub struct LinkedListAllocator {
    head: ListNode
}

impl LinkedListAllocator {
    /// Searches the free list to find free memory of size `size`
    unsafe fn find_free_region(&mut self, size: usize) -> Option<*mut u8> {
        let size = size as u64;
        let mut node_ptr_opt: Option<*mut ListNode> = Some(&mut self.head as *mut _);
        while let Some(curr_node_ptr) = (*node_ptr_opt.unwrap()).next {
            // Perfect fit
            if (*curr_node_ptr).size == size {
                mem::swap(&mut (*(node_ptr_opt.unwrap())).next, &mut (*curr_node_ptr).next);
                return Some((*curr_node_ptr).start_addr().as_mut_ptr());
            } else if (*curr_node_ptr).size > size {
                // Bigger
                let mut new_node_ptr = ((*curr_node_ptr).start_addr() + size).as_u64() as *mut ListNode;
                (*new_node_ptr).size = (*curr_node_ptr).size - size;
                (*new_node_ptr).next = (*curr_node_ptr).next;
                (*node_ptr_opt.unwrap()).next = Some(new_node_ptr);
                return Some((*curr_node_ptr).start_addr().as_mut_ptr());
            }
            node_ptr_opt = (*node_ptr_opt.unwrap()).next;
        }
        None
    }
    
    /// Adds a free region to the list
    /// Merges adjacent free regions
    unsafe fn add_free_region(&mut self, mem_chunk: MemChunk) { 
        let mut node_ptr_opt: Option<*mut ListNode> = Some(&mut self.head as *mut _);
        while let Some(curr_node_ptr) = node_ptr_opt {
            // The mem chunk comes immediately after the node
            // ----NNNNN--------...
            // ---------MMMM----
            if (*curr_node_ptr).end_addr() == mem_chunk.start_addr() {
                // Merging the regions
                (*curr_node_ptr).size += mem_chunk.size();
                return;
            } else if ((*curr_node_ptr).next.is_some() && mem_chunk.end_addr() < (*(*curr_node_ptr).next.unwrap()).start_addr())
                || (*curr_node_ptr).next.is_none() {
                // The mem chunk comes after the node but before the next
                // ----NNNN---------NNNNN-----
                // ----------MMM--------------
                //
                // The mem chunk comes after the node and there is no other node after
                // ----NNNN--------------
                // ----------MMM---------
                let new_node_ptr = mem_chunk.start_addr().as_u64() as *mut ListNode;
                *new_node_ptr = ListNode { size: mem_chunk.size(), next: (*curr_node_ptr).next.take() };
                (*curr_node_ptr).next = Some(new_node_ptr);
                return;
            } else if (*curr_node_ptr).next.is_some() && mem_chunk.end_addr() == (*(*curr_node_ptr).next.unwrap()).start_addr() {
                // The mem chunk come immediately before the next node
                // ------NNNN----------NNNN-----
                // ---------------MMMMM---------
                let mut new_node_ptr = mem_chunk.start_addr().as_u64() as *mut ListNode;
                let next_node_ptr = (*curr_node_ptr).next.unwrap();
                (*new_node_ptr).size = (*next_node_ptr).size + mem_chunk.size();
                (*new_node_ptr).next = (*next_node_ptr).next;
                (*curr_node_ptr).next = Some(new_node_ptr);
                return;
            }
            // Because of the way the nodes in the list are considered, the
            // start address of the mem chunk can't be lesser than the start address of the
            // curr_node. The head node's start address is always 0, and curr_node starts from
            // the head node. Since mem chunk's address will always be greater than 0, the start
            // address of the mem chunk will surely come after the head node. So if it comes before
            // the head.next node, it will still come after the head, which is the first node being
            // considered.
            // Also, the mem chunk's start address can fall in any node's range, because then
            // it will be part of that node.
            node_ptr_opt = (*curr_node_ptr).next;
        }
    }

    unsafe fn iter(&self) -> FreeRegionIter {
        if self.head.next.is_some() {
            FreeRegionIter {
                curr_node: Some(self.head.next.unwrap())
            }
        } else {
            FreeRegionIter {
                curr_node: None
            }
        }
    }
}

struct FreeRegionIter {
    curr_node: Option<*mut ListNode>
}

impl Iterator for FreeRegionIter {
    type Item = MemChunk;
    fn next(&mut self) -> Option<Self::Item> {
        let curr_node = self.curr_node;
        if let Some(node) = curr_node {
            unsafe {
                self.curr_node = (*node).next;
                Some(MemChunk {
                    start_addr: (*node).start_addr(),
                    size: (*node).size
                })
            }
        } else {
            None
        }
    }
}

unsafe impl Allocator for Mutex<LinkedListAllocator> {
    unsafe fn alloc(&self, size_of_type: usize, size_to_alloc: usize) -> Result<*mut u8, Error> {
        if let Some(mem_ptr) = self.lock().find_free_region(size_of_type * size_to_alloc) {
            Ok(mem_ptr)
        } else {
            Err(Error::AllocationError)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, size_to_dealloc: usize)  -> Result<(), Error> {
        self.lock().add_free_region(MemChunk {
            start_addr: Addr::from_ptr(ptr),
            size: size_to_dealloc as u64
        });
        Ok(())
    }
}

#[cfg(test)]
#[allow(unused_variables)]
mod tests {
    use super::*;
    use crate::vec::Vec;
    use std::vec::Vec as StdVec;
    use std::mem::ManuallyDrop;
    const FOUR_KIB: usize = 2usize.pow(12);

    #[test]
    fn test_vec_create() {
        let allocator = Mutex::new(get_4kib_allocator());
        let vec_size = 20;
        let v: Vec<u8> = Vec::with_capacity(vec_size, &allocator);
        unsafe {
            let new_heap_size = (*allocator.lock().head.next.unwrap()).size;
            assert_eq!(new_heap_size as usize, FOUR_KIB - vec_size);
        }
    }

    #[test]
    fn test_vec_drop() {
        let allocator = Mutex::new(get_4kib_allocator());
        let vec_size = 20;
        {
            let v: Vec<u8> = Vec::with_capacity(vec_size, &allocator);
        }
        unsafe {
            let heap_size_after_dropping_vec = (*allocator.lock().head.next.unwrap()).size;
            assert_eq!(heap_size_after_dropping_vec, FOUR_KIB as u64);
        }
    }

    #[test]
    #[should_panic]
    fn test_vec_too_big() {
        let allocator = Mutex::new(get_4kib_allocator());
        let vec_size = FOUR_KIB + 1;
        let v: Vec<u8> = Vec::with_capacity(vec_size, &allocator);
        
    }

    #[test]
    fn test_iter1() {
        let allocator = get_4kib_allocator();
        let mut iter = unsafe { allocator.iter() };
        if let Some(MemChunk { size, .. }) = iter.next() {
            assert_eq!(size as usize, FOUR_KIB);
        }
        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_iter2() {
        let allocator = Mutex::new(get_4kib_allocator());
        // 4 items of 4 bytes each
        let allocd_ptr = unsafe { allocator.alloc(4, 4).unwrap() };
        let mut iter = unsafe { allocator.lock().iter() };
        if let Some(MemChunk { size, .. }) = iter.next() {
            assert_eq!(size as usize, FOUR_KIB - 4 * 4);
        }
        assert_eq!(None, iter.next());

        unsafe { allocator.dealloc(allocd_ptr, 4 * 4).unwrap() };
        let mut iter = unsafe { allocator.lock().iter() };
        if let Some(MemChunk { size, .. }) = iter.next() {
            assert_eq!(size as usize, FOUR_KIB);
        }
        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_iter3() {
        #[derive(Clone)]
        struct Struct(u32, u32, u32);
        let allocator = Mutex::new(get_4kib_allocator());
        let v: Vec<Struct> = Vec::with_capacity(5, &allocator);
        let mut iter = unsafe { allocator.lock().iter() };
        if let Some(MemChunk { size, .. }) = iter.next() {
            assert_eq!(size as usize, FOUR_KIB - 5 * mem::size_of::<Struct>());
        }
        assert_eq!(None, iter.next());
        mem::drop(v);
        let mut iter = unsafe { allocator.lock().iter() };
        if let Some(MemChunk { size, .. }) = iter.next() {
            assert_eq!(size as usize, FOUR_KIB);
        }
        assert_eq!(None, iter.next());
    }

    fn get_4kib_allocator() -> LinkedListAllocator {
        let mem: ManuallyDrop<StdVec<u8>> = ManuallyDrop::new(StdVec::with_capacity(FOUR_KIB));
        let mem_ptr = mem.as_ptr() as *mut u8;
        let mut allocator = LinkedListAllocator {
            head: ListNode {
                size: 0,
                next: None
            }
        };
        unsafe {
            allocator.add_free_region(MemChunk {
                start_addr: Addr::from_ptr(mem_ptr),
                size: FOUR_KIB as u64
            });
        }
        return allocator;
    }

}
