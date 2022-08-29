//! A library for adding event handlers

#![no_std]

use core::ops::{Index, IndexMut};
use drivers::keyboard::{KeyCode, KeyDirection, KeyModifiers};
use machine::instructions::interrupts::without_interrupts;
use collections::vec::Vec;
use collections::allocator::get_allocator;
use collections::boxed::Box;
use lazy_static::lazy_static;
use sync::mutex::Mutex;
use printer::println;

const NO_OF_EVENTS: u8 = 1;


lazy_static! {
    pub static ref EVENT_HOOKER: Mutex<EventHooker> = Mutex::new(EventHooker::new());
}

pub fn hook_event(event: Event, f: fn(Event)) {
    without_interrupts(|| {
        EVENT_HOOKER.lock().hook_event(event, f);
    });
}

pub fn send_event(event: Event) {
    without_interrupts(|| {
        EVENT_HOOKER.lock().send_event(event);
    });
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Timer,
    Keyboard(KeyCode, KeyDirection, KeyModifiers)
}

impl Event {
    /// Creates a random instance of Keyboard when the specific instance doesn't matter
    pub fn keyboard() -> Self {
        Event::Keyboard(KeyCode::Escape, KeyDirection::Up, KeyModifiers::new())
    }
}

/// Acts as mediator between the interrupt service routines and the game code
pub struct EventHooker {
    handlers: [Vec<'static, Handler>; 2],
    next_idx: usize
}

unsafe impl Send for EventHooker {}

impl EventHooker {
    /// Index into the handlers field for timer handlers
    const TIMER_INDEX: usize = 0;
    /// Index into the handlers field for keyboard handlers
    const KEYBOARD_INDEX: usize = 1;

    /// Creates a new empty EventHooker
    pub fn new() -> Self {
        use core::mem;
        println!("{}", mem::size_of::<Handler>());
        EventHooker {
            handlers: [Vec::with_capacity(1, get_allocator()), Vec::with_capacity(1, get_allocator())],
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
    pub fn hook_event(&mut self, event: Event, f: fn(Event)) -> usize {
        let next_idx = self.next_idx;
        self[event].push(Handler { idx: next_idx, func: f });
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
    /// use event_hook::{EventHooker, Event};
    /// let mut event_hooker = EventHooker::new();
    /// let idx = event_hooker.hook_event(Event::Timer, || ());
    /// let unhook_result = event_hooker.unhook_event(idx, Event::Timer);
    /// assert_eq!(unhook_result, Ok(()));
    /// let unhook_result = event_hooker.unhook_event(idx, EventTimer);
    /// ```
    pub fn unhook_event(&mut self, idx: usize, event: Event) -> Result<(), Error> {
        for (i, handler) in self[event].iter_mut().enumerate() {
            if let Handler {idx, func} = handler {
                self[event].remove(i);
                return Ok(());
            }
        }
        Err(Error::IdxNotFound)
    }
}

impl Index<Event> for EventHooker {
    type Output = Vec<'static, Handler>;

    fn index(&self, event: Event) -> &Self::Output {
        match event {
            Event::Timer => &self.handlers[Self::TIMER_INDEX],
            Event::Keyboard(_, _, _) => &self.handlers[Self::KEYBOARD_INDEX]
        }
        
    }
}

impl IndexMut<Event> for EventHooker {
    fn index_mut(&mut self, event: Event) -> &mut Self::Output {
        match event {
            Event::Timer => &mut self.handlers[Self::TIMER_INDEX],
            Event::Keyboard(_, _, _) => &mut self.handlers[Self::KEYBOARD_INDEX]
        }
    }
}

/// A unique function in an vector associated with a particular event
#[derive(Clone)]
pub struct Handler {
    /// A unique number in the vector associated with the handler.
    /// Used to identify the handler when removing handlers
    idx: usize,
    /// A function that is executed whenever the associated event is sent
    func: fn(Event)
}

pub enum Error {
    /// Returned when unhook_event is called with a non existent idx
    IdxNotFound
}