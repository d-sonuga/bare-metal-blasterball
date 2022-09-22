//! Macro for creating lazily evaluated statics

#![cfg_attr(not(test), no_std)]
#![feature(custom_test_frameworks)]
#![cfg_attr(test, test_runner(tester::test_runner))]

pub use core::ops::Deref;
use sync::once::Once;

#[cfg(test)]
mod tests;

#[macro_export(local_inner_macros)]
macro_rules! lazy_static_main {
    ($(#[$attr:meta])* ($($visibility:tt)*) static ref $name:ident : $item_type:ty = $e:expr;) => {
        // Make the $name identifier into a type of its own
        $(#[$attr])*
        $($visibility)* struct $name {__private_field: ()}
        $($visibility)* static $name: $name = $name {__private_field: ()};
        
        // $name then derefs to the value of the expression $e, which is lazily
        // evaluated
        impl $crate::Deref for $name {
            type Target = $item_type;

            fn deref(&self) -> &$item_type {
                // The initialization function which returns the expression
                #[inline(always)]
                fn __static_ref_init() -> $item_type { $e }
                
                #[inline(always)]
                fn __stability() -> &'static $item_type {
                    static LAZY: $crate::Lazy<$item_type> = $crate::Lazy::INIT;
                    LAZY.get(__static_ref_init)
                }
                __stability()
            }
        }
    }
}

#[macro_export(local_inner_macros)]
macro_rules! lazy_static {
    ($(#[$attr:meta])* static ref $name:ident : $item_type:ty = $e:expr;) => {
        lazy_static_main!($(#[$attr])* () static ref $name : $item_type = $e;);
    };
    ($(#[$attr:meta])* pub static ref $name:ident : $item_type:ty = $e:expr;) => {
        lazy_static_main!($(#[$attr])* (pub) static ref $name : $item_type = $e;);
    };
}

/// A primitive for creating lazily evaluated values
pub struct Lazy<T: Sync>(Once<T>);

impl<T: Sync> Lazy<T> {
    pub const INIT: Self = Lazy(Once::INIT);

    /// To get the value that is being held
    ///
    /// The function initializer is called only once when the value is accessed
    /// for the first time
    pub fn get<F: FnOnce() -> T>(&'static self, initializer: F) -> &T {
        self.0.call_once(initializer)
    }
}
