//! A library for adding event handlers

#![cfg_attr(not(test), no_std)]
#![feature(unboxed_closures, fn_traits)]

use core::ops::{Index, IndexMut};
use core::clone::Clone;
use core::marker::PhantomData;
use machine::keyboard::{KeyCode, KeyDirection, KeyModifiers};
use machine::instructions::interrupts::without_interrupts;
use collections::vec::Vec;
use collections::vec;
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
    Keyboard(KeyCode, KeyDirection, KeyModifiers),
    Sound
}

#[derive(Clone, Copy, Debug)]
pub enum EventKind {
    Timer,
    Keyboard,
    Sound
}

impl EventKind {
    fn from_event(event: Event) -> Self {
        match event {
            Event::Timer => EventKind::Timer,
            Event::Keyboard(_, _, _) => EventKind::Keyboard,
            Event::Sound => EventKind::Sound
        }
    }
}

/// Acts as mediator between the interrupt service routines and the game code
///
/// # Synchronization
///
/// When an interrupts occurs, a send_event is called. And all
/// the functions associated with a particular event are called.
/// During this period of calling, no other code runs, so nothing
/// can interrupt the functions while they are called, but while the
/// send_event function is running, the functions being executed can
/// can run `hook_event` or `unhook_event`, so the EventHooker instance can be
/// modified when the `send_event` function is executing.
///
/// When a hook_event is called, an interrupt can occur at any point in time
/// between the adding of the function to the handlers vector and the send_event
/// calling all the functions. The send_event should not be called while a
/// function is being hooked, so the handlers vector push should be atomic 
/// in the sense that a handler is either in or out of the vector; no partially
/// initialized state
///
/// When an unhook_event is called, an interrupt can occur at any point in time
/// between the removal of the function from the handlers vector and the send_event
/// calling all the functions. The send_event should not be called while a function is
/// being unhooked, so the handlers vector remove should be atomic in the sense that
/// a handler is either in or out of the vector; no partially removed state
///
/// From the info above, only the vector needs a mutex, not the whole EventHooker
/// instance.
/// The `hook_event` function writes to the handlers vector and the `send_event`
/// function reads the vector. These 2 actions cannot occur at the same time.
/// It can be resolved by adding 3 new queues: `missed_events`, `missed_hooks` and `missed_unhooks`.
///
/// When hook_event is called, if the handlers vector is locked, it places the
/// missed_hook in `missed_hooks`
/// When send_event is called, if the handlers vector is locked, it places the
/// missed event in `missed_events`
/// When unhook_event is called, if the handlers vector is locked, it places the
/// missed unhook in 'missed_unhooks`
///
/// To some how this will work, consider the following cases:
///
/// # Case 1: `send_event` is called and handlers is locked
/// In this case, the handlers have already been locked by a hook_event / unhook_event
/// call, so the handlers vector is being modified.
/// The event that is being sent is just enqueued on the `missed_events`
/// queue.
/// ### But how will the event eventually be sent?
/// At the end of the `hook_event` execution, before the handlers lock is released,
/// the `missed_unhooks` queue is checked for any missed unhooks. If there are any,
/// they are removed from the handlers.
/// The same is done for the `missed_events` queue. It is checked for any missed events. If
/// there are any they are executed and the handlers lock is released. If there aren't any, the
/// handlers lock is released.
/// ### What if the `send_event` is called again while those missed functions are executing?
/// The handlers vector will be found to be locked again and the `missed_events` queue will
/// get the missed events enqueued on it.
/// This works because the `hook_event` or execution of handlers does not involve
/// writing to the `missed_events` or `missed_unhooks` queues. Only `send_event` writes to the
/// `missed_events` queue, only hook_event writes to the `missed_hooks` queue and only
/// unhook_event writes to the `missed_unhooks` queue.
/// 
/// # Case 2: `hook_event` is called and handlers is locked
/// In this case, the handlers have already been locked by `send_event` / `unhook_event`. So the
/// handlers vector is being read from or written to.
/// The function will be enqueued on the `missed_hooks` queue.
/// ### But how will the hook eventually be written to the handlers?
/// At the end of `send_event`'s execution, before the handlers lock is released, the
/// `missed_hooks` queue is checked for any missed hooks. If there are any, they are
/// written to the handlers vector and the handlers lock is released. If there aren't any,
/// the handlers lock is released. The same goes for the `unhook_event`'s execution.
/// 
/// # Case 3: `unhook_event` is called and handlers is locked
/// In this case, the handlers have already been locked by `send_event` / `hook_event`. So
/// the handlers vector is being read from or written to.
/// The function will be enqueued on the `missed_unhooks` queue.
/// ### But how will the hook eventually be removed from the handlers?
/// At the end of `send_event` execution, before the handlers lock is released,
/// the `missed_unhooks` queue is checked for any missed unhooks. If there are any, they are
/// written to the handlers vector and the handlers lock is released. If there aren't any,
/// the handlers lock is released. The same goes for the `hook_event`'s execution.
pub struct EventHooker<'a> {
    /// The functions to be called when events take place
    handlers: [Vec<'a, Handler<'a>>; 3],
    /// The next id to be used as a handler idx
    next_idx: HandlerId
}

unsafe impl<'a> Send for EventHooker<'a> {}

impl<'a> EventHooker<'a> {
    /// Index into the handlers field for timer handlers
    const TIMER_INDEX: usize = 0;
    /// Index into the handlers field for keyboard handlers
    const KEYBOARD_INDEX: usize = 1;
    /// Index into the handlers field for sound handlers
    const SOUND_INDEX: usize = 2;

    /// Creates a new empty EventHooker
    pub fn new(allocator: &'a dyn Allocator) -> Self {
        EventHooker {
            handlers: [
                Vec::with_capacity(1, allocator),
                Vec::with_capacity(1, allocator),
                Vec::with_capacity(1, allocator)
            ],
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
    /// assert_eq!(idx, 0);
    /// ```
    ///
    /// # Panics
    /// In the rare, if not impossible, occasion where next_idx reaches the max
    ///
    /// # Safety
    ///
    /// This function is highly unsafe. The BoxedFn can be a sort of trojan horse of
    /// unsafety because there is no way to tell if the closure or function in it
    /// takes any reference that doesn't live long enough or performs any unsafe
    /// operations. Anything that `f` performs is completely opaque, with no way
    /// to verify its safety
    pub fn hook_event(&mut self, event_kind: EventKind, f: BoxedFn<'static>) -> usize {
        let next_idx = self.next_idx;
        self[event_kind].push(Handler { idx: next_idx, func: f });
        self.next_idx += 1;
        if self.next_idx == usize::MAX {
            panic!("next_idx has reached max");
        }
        next_idx
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
            if handler.idx == idx {
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
            EventKind::Keyboard => &self.handlers[Self::KEYBOARD_INDEX],
            EventKind::Sound => &self.handlers[Self::SOUND_INDEX]
        }
        
    }
}

impl<'a> IndexMut<EventKind> for EventHooker<'a> {
    fn index_mut(&mut self, event: EventKind) -> &mut Self::Output {
        match event {
            EventKind::Timer => &mut self.handlers[Self::TIMER_INDEX],
            EventKind::Keyboard => &mut self.handlers[Self::KEYBOARD_INDEX],
            EventKind::Sound => &mut self.handlers[Self::SOUND_INDEX]
        }
    }
}

type HandlerId = usize;

/// A unique function in an vector associated with a particular event
#[derive(Clone, Debug)]
pub struct Handler<'a> {
    /// A unique number in the vector associated with the handler.
    /// Used to identify the handler when removing handlers
    idx: HandlerId,
    /// A function that is executed whenever the associated event is sent
    func: BoxedFn<'a>,
}

#[derive(Debug, PartialEq)]
pub enum Error {
    /// Returned when unhook_event is called with a non existent idx
    IdxNotFound
}

#[cfg(test)]
mod tests {
    use super::*;
    use collections::allocator::{Allocator, Error};
    use std::vec::Vec as StdVec;
    use core::mem::ManuallyDrop;
    use core::mem;
    use crate::box_fn;
    
    #[test]
    fn test_unhook_only_removes_one_handler() {
        let mut event_hooker = EventHooker::new(&AlwaysSuccessfulAllocator);
        assert_eq!(event_hooker[EventKind::Timer].len(), 0);
        let idx = event_hooker.hook_event(EventKind::Timer, box_fn!(|_| (), &AlwaysSuccessfulAllocator));
        assert_eq!(event_hooker[EventKind::Timer].len(), 1);
        let other_idx = event_hooker.hook_event(EventKind::Timer, box_fn!(|_| (), &AlwaysSuccessfulAllocator));
        assert_eq!(event_hooker[EventKind::Timer].len(), 2);
        event_hooker.unhook_event(other_idx, EventKind::Timer);
        assert_eq!(event_hooker[EventKind::Timer].len(), 1);
        assert_eq!(event_hooker[EventKind::Timer][0].idx, idx);
    }

    struct AlwaysSuccessfulAllocator;
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