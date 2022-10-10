//! A library for adding event handlers

#![cfg_attr(not(test), no_std)]
#![feature(unboxed_closures, fn_traits)]

use core::ops::{Index, IndexMut};
use core::clone::Clone;
use core::marker::PhantomData;
use machine::keyboard::{KeyCode, KeyDirection, KeyModifiers};
use machine::instructions::interrupts::without_interrupts;
use collections::vec::Vec;
use collections::allocator::{get_allocator, Allocator};
use lazy_static::lazy_static;
use sync::mutex::Mutex;
use artist::println;

pub mod boxed_fn;
use boxed_fn::BoxedFn;

const NO_OF_EVENTS: u8 = 1;


lazy_static! {
    pub static ref EVENT_HOOKER: Mutex<EventHooker<'static>> = Mutex::new(EventHooker::new(get_allocator()));
}

pub fn hook_event(event: EventKind, f: BoxedFn<'static>) -> usize {
    without_interrupts(|| {
        EVENT_HOOKER.lock().hook_event(event, f)
    })
}

pub fn unhook_event(event_id: usize, event_kind: EventKind) -> Result<(), Error> {
    without_interrupts(|| {
        EVENT_HOOKER.lock().unhook_event(event_id, event_kind)
    })
}

pub fn send_event(event: Event) {
    without_interrupts(|| {
        EVENT_HOOKER.lock().send_event(event);
    });
}

pub fn unhook_all_events(event_kind: EventKind) {
    without_interrupts(|| {
        EVENT_HOOKER.lock().unhook_all_events(event_kind)
    });
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Timer,
    Keyboard(KeyCode, KeyDirection, KeyModifiers)
}

#[derive(Clone, Copy, Debug)]
pub enum EventKind {
    Timer,
    Keyboard
}

impl EventKind {
    fn from_event(event: Event) -> Self {
        match event {
            Event::Timer => EventKind::Timer,
            Event::Keyboard(_, _, _) => EventKind::Keyboard
        }
    }
}

/// Acts as mediator between the interrupt service routines and the game code
pub struct EventHooker<'a> {
    handlers: [Vec<'a, Handler<'a>>; 2],
    next_idx: usize
}

unsafe impl<'a> Send for EventHooker<'a> {}

impl<'a> EventHooker<'a> {
    /// Index into the handlers field for timer handlers
    const TIMER_INDEX: usize = 0;
    /// Index into the handlers field for keyboard handlers
    const KEYBOARD_INDEX: usize = 1;

    /// Creates a new empty EventHooker
    pub fn new(allocator: &'a dyn Allocator) -> Self {
        EventHooker {
            handlers: [Vec::with_capacity(1, allocator), Vec::with_capacity(1, allocator)],
            next_idx: 0
        }
    }

    /// Registers a function `f` to be invoked when event is sent.
    /// Returns the index of the function in the list of handlers
    /// which can be used to unhook the function.
    ///
    /// Takes O(1) time since it's just appending to a vector
    ///
    /// # Example
    ///
    /// ```
    /// use collections::allocator::{Allocator, Error};
    /// use std::vec::Vec as StdVec;
    /// use core::mem::ManuallyDrop;
    /// use core::mem;
    /// use event_hook::{EventHooker, Event, EventKind};
    /// use event_hook::boxed_fn::BoxedFn;
    ///
    /// pub struct AlwaysSuccessfulAllocator;
    /// unsafe impl Allocator for AlwaysSuccessfulAllocator {
    ///     unsafe fn alloc(&self, size_of_type: usize, size_to_alloc: usize) -> Result<*mut u8, Error> {
    ///         let mut v: ManuallyDrop<StdVec<u8>> = ManuallyDrop::new(StdVec::with_capacity(size_of_type * size_to_alloc));
    ///         Ok(v.as_mut_ptr() as *mut u8)
    ///     }
    ///     unsafe fn dealloc(&self, ptr: *mut u8, size_to_dealloc: usize)  -> Result<(), Error> {
    ///         let v: StdVec<u8> = StdVec::from_raw_parts(ptr, size_to_dealloc, size_to_dealloc);
    ///         mem::drop(v);
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut event_hooker = EventHooker::new(&AlwaysSuccessfulAllocator);
    /// let idx = event_hooker.hook_event(EventKind::Timer, BoxedFn::new(|_| (), &AlwaysSuccessfulAllocator));
    /// //assert_eq!(idx, 0);
    /// ```
    ///
    /// # Panics
    /// In the rare, if not impossible, occasion where next_idx reaches the max
    pub fn hook_event(&mut self, event_kind: EventKind, f: BoxedFn<'static>) -> usize {
        let next_idx = self.next_idx;
        self[event_kind].push(Handler { idx: next_idx, func: f });
        self.next_idx += 1;
        if self.next_idx == usize::MAX {
            panic!("next_idx has reached max");
        }
        self.next_idx - 1
    }

    /// Invokes all functions hooked to event
    ///
    /// Takes O(nm) time where n is the number of functions in `event`'s vector and m is
    /// the running time of the longest running function, since it is invoking
    /// all functions in `event`'s vector.
    ///
    /// # Example
    ///
    /// ```
    /// use collections::allocator::{Allocator, Error};
    /// use std::vec::Vec as StdVec;
    /// use core::mem::ManuallyDrop;
    /// use core::mem;
    /// use event_hook::{EventHooker, Event, EventKind};
    /// use event_hook::boxed_fn::BoxedFn;
    ///
    /// pub struct AlwaysSuccessfulAllocator;
    /// unsafe impl Allocator for AlwaysSuccessfulAllocator {
    ///     unsafe fn alloc(&self, size_of_type: usize, size_to_alloc: usize) -> Result<*mut u8, Error> {
    ///         let mut v: ManuallyDrop<StdVec<u8>> = ManuallyDrop::new(StdVec::with_capacity(size_of_type * size_to_alloc));
    ///         Ok(v.as_mut_ptr() as *mut u8)
    ///     }
    ///     unsafe fn dealloc(&self, ptr: *mut u8, size_to_dealloc: usize)  -> Result<(), Error> {
    ///         let v: StdVec<u8> = StdVec::from_raw_parts(ptr, size_to_dealloc, size_to_dealloc);
    ///         mem::drop(v);
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut event_hooker = EventHooker::new(&AlwaysSuccessfulAllocator);
    /// let mut x = 1;
    /// event_hooker.hook_event(EventKind::Timer, BoxedFn::new(|_| x += 1, &AlwaysSuccessfulAllocator));
    /// event_hooker.send_event(Event::Timer);
    /// assert_eq!(x, 2);
    /// ```
    pub fn send_event(&self, event: Event) {
        let event_kind = EventKind::from_event(event);
        for i in 0..self[event_kind].len() {
            let handler = &self[event_kind][i];
            (handler.func)(event);
        }
    }

    /// Removes a function with id idx related to a particular event.
    /// If there is no function with id idx, an error is returned
    ///
    /// Takes O(n) time, where n is the number of functions in the `event`'s vector because
    /// removing a function in an arbitrary position requires all the functions that come after
    /// to be shifted backwards.
    ///
    /// # Example
    ///
    /// ```
    /// use collections::allocator::{Allocator, Error};
    /// use std::vec::Vec as StdVec;
    /// use core::mem::ManuallyDrop;
    /// use core::mem;
    /// use event_hook::{EventHooker, Event, EventKind, Error as EventHookError};
    /// use event_hook::boxed_fn::BoxedFn;
    ///
    /// pub struct AlwaysSuccessfulAllocator;
    /// unsafe impl Allocator for AlwaysSuccessfulAllocator {
    ///     unsafe fn alloc(&self, size_of_type: usize, size_to_alloc: usize) -> Result<*mut u8, Error> {
    ///         let mut v: ManuallyDrop<StdVec<u8>> = ManuallyDrop::new(StdVec::with_capacity(size_of_type * size_to_alloc));
    ///         Ok(v.as_mut_ptr() as *mut u8)
    ///     }
    ///     unsafe fn dealloc(&self, ptr: *mut u8, size_to_dealloc: usize)  -> Result<(), Error> {
    ///         let v: StdVec<u8> = StdVec::from_raw_parts(ptr, size_to_dealloc, size_to_dealloc);
    ///         mem::drop(v);
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut event_hooker = EventHooker::new(&AlwaysSuccessfulAllocator);
    /// let mut x = 1;
    /// let idx = event_hooker.hook_event(EventKind::Timer, BoxedFn::new(|_| x += 1, &AlwaysSuccessfulAllocator));
    /// let unhook_result = event_hooker.unhook_event(idx, EventKind::Timer);
    /// assert_eq!(unhook_result, Ok(()));
    /// let unhook_result = event_hooker.unhook_event(idx, EventKind::Timer);
    /// assert_eq!(unhook_result, Err(EventHookError::IdxNotFound));
    /// ```
    pub fn unhook_event(&mut self, idx: usize, event_kind: EventKind) -> Result<(), Error> {
        for i in 0..self[event_kind].len() {
            let mut handler = &mut self[event_kind][i];
            if let Handler {idx, func, ..} = handler {
                self[event_kind].remove(i);
                return Ok(());
            }
        }
        Err(Error::IdxNotFound)
    }

    /// Removes all functions related to a particular event.
    ///
    /// Takes O(n) time, where n is the number of functions in the `event`'s vector because
    /// removing a function in an arbitrary position requires all the functions that come after
    /// to be shifted backwards.
    ///
    /// # Example
    ///
    /// ```
    /// use collections::allocator::{Allocator, Error};
    /// use std::vec::Vec as StdVec;
    /// use core::mem::ManuallyDrop;
    /// use core::mem;
    /// use event_hook::{EventHooker, Event, EventKind};
    /// use event_hook::boxed_fn::BoxedFn;
    ///
    /// pub struct AlwaysSuccessfulAllocator;
    /// unsafe impl Allocator for AlwaysSuccessfulAllocator {
    ///     unsafe fn alloc(&self, size_of_type: usize, size_to_alloc: usize) -> Result<*mut u8, Error> {
    ///         let mut v: ManuallyDrop<StdVec<u8>> = ManuallyDrop::new(StdVec::with_capacity(size_of_type * size_to_alloc));
    ///         Ok(v.as_mut_ptr() as *mut u8)
    ///     }
    ///     unsafe fn dealloc(&self, ptr: *mut u8, size_to_dealloc: usize)  -> Result<(), Error> {
    ///         let v: StdVec<u8> = StdVec::from_raw_parts(ptr, size_to_dealloc, size_to_dealloc);
    ///         mem::drop(v);
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut event_hooker = EventHooker::new(&AlwaysSuccessfulAllocator);
    /// let idx = event_hooker.hook_event(EventKind::Timer, BoxedFn::new(|_| (), &AlwaysSuccessfulAllocator));
    /// assert_eq!(event_hooker[EventKind::Timer].len(), 1);
    /// event_hooker.unhook_all_events(EventKind::Timer);
    /// assert_eq!(event_hooker[EventKind::Timer].len(), 0);
    /// ```
    pub fn unhook_all_events(&mut self, event_kind: EventKind) {
        for _ in 0..self[event_kind].len() {
            self[event_kind].pop();
        }
    }
}

impl<'a> Index<EventKind> for EventHooker<'a> {
    type Output = Vec<'a, Handler<'a>>;

    fn index(&self, event: EventKind) -> &Self::Output {
        match event {
            EventKind::Timer => &self.handlers[Self::TIMER_INDEX],
            EventKind::Keyboard => &self.handlers[Self::KEYBOARD_INDEX]
        }
        
    }
}

impl<'a> IndexMut<EventKind> for EventHooker<'a> {
    fn index_mut(&mut self, event: EventKind) -> &mut Self::Output {
        match event {
            EventKind::Timer => &mut self.handlers[Self::TIMER_INDEX],
            EventKind::Keyboard => &mut self.handlers[Self::KEYBOARD_INDEX]
        }
    }
}

/// A unique function in an vector associated with a particular event
#[derive(Clone, Debug)]
pub struct Handler<'a> {
    /// A unique number in the vector associated with the handler.
    /// Used to identify the handler when removing handlers
    idx: usize,
    /// A function that is executed whenever the associated event is sent
    func: BoxedFn<'a>,
}

#[derive(Debug, PartialEq)]
pub enum Error {
    /// Returned when unhook_event is called with a non existent idx
    IdxNotFound
}
