//! A library for adding event handlers

#![no_std]

use core::ops::{Index, IndexMut};
use drivers::keyboard::{Keycode, KeyDirection, KeyModifiers};
use machine::instructions::interrupts::without_interrupts;
use alloc::vec::Vec;

const NO_OF_EVENTS: u8 = 1;


lazy_static! {
    pub static ref EVENT_HOOKER: Mutex<EventHooker> = Mutex::new(EventHooker::new());
}

pub fn hook_event(event: Event, f: F) where F: Fn() {
    without_interrupts(||{
        EVENT_HOOKER.lock().hook_event(event, f);
    });
}

pub fn send_event(event: Event) {
    without_interrupts(|| {
        EVENT_HOOKER.lock().send_event(event);
    });
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Event {
    Timer = 0,
    Keyboard(KeyCode, KeyDirection, KeyModifiers)
}

/// Acts as mediator between the interrupt service routines and the game code
pub struct EventHooker<F: Fn()> {
    handlers: [Vec<Handler<F>>; 2],
    next_idx: usize
}

impl<F> EventHooker<F> {

    /// Creates a new empty EventHooker
    pub fn new() -> Self {
        EventHooker {
            handlers: [vec![], vec![]],
            next_idx: 0
        }
    }

    /// Registers a function f to be invoked when event is sent.
    /// Returns the index of the function in the list of handlers
    /// which can be used to unhook the function.
    ///
    /// Takes O(1) time since it's just appending to a vector
    ///
    /// # Example
    ///
    /// ```
    /// use event_hook::{EventHooker, Event};
    /// let mut event_hooker = new EventHooker();
    /// let idx = event_hooker.hook_event(Event::Timer, || ());
    /// assert_eq!(idx, 0);
    /// ```
    ///
    /// # Panics
    /// In the rare, if not impossible, occasion where next_idx reaches the max
    pub fn hook_event(&mut self, event: Event, f: F) -> usize {
        self[event].push(Handler::new(self.next_idx, f));
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
    /// use event_hook::{EventHooker, Event};
    /// let mut x = 1;
    /// let event_hooker = EventHooker::new();
    /// event_hooker.hook_event(Event::Timer, || x += 1);
    /// event_hooker.send_event(Event::Timer);
    /// assert_eq!(x, 2);
    /// ```
    pub fn send_event(&self, event: Event) {
        for handler in self[event].iter() {
            handler.func();
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
    /// use event_hook::{EventHooker, Event};
    /// let mut event_hooker = EventHooker::new();
    /// let idx = event_hooker.hook_event(Event::Timer, || ());
    /// let unhook_result = event_hooker.unhook_event(idx, Event::Timer);
    /// assert_eq!(unhook_result, Ok(()));
    /// let unhook_result = event_hooker.unhook_event(idx, EventTimer);
    pub fn unhook_event(&mut self, idx: usize, event: Event) -> Result<(), Error> {
        for (i, handler) in self.handlers[event].iter_mut().enumerate() {
            if Handler {idx, _} = handler {
                self.handlers[event].remove(i);
                return Ok(());
            }
        }
        Err(Error::IdxNotFound)
    }
}

impl<F> Index<Event> for EventHooker<F> {
    type Output = Vec<F>;

    fn index(&self, index: Event) -> &Self::Output {
        &self.handlers[index as u8 as usize]
    }
}

impl<F> IndexMut<Event> for EventHook<F> {
    fn index_mut(&mut self, index: Event) -> &mut Self::Output {
        &mut self.handlers[index as u8 as usize]
    }
}

/// A unique function in an vector associated with a particular event
struct Handler<F> {
    /// A unique number in the vector associated with the handler.
    /// Used to identify the handler when removing handlers
    idx: usize,
    /// A function that is executed whenever the associated event is sent
    func: F
}

enum Error {
    /// Returned when unhook_event is called with a non existent idx
    IdxNotFound
}