#![cfg_attr(not(test), no_std)]

#[cfg(test)]
use std::ops::Fn;

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]){
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
}

pub trait Testable {
    fn run(&self);
}

#[cfg(test)]
impl<T> Testable for T where T: Fn() {
    fn run(&self){
        println!("{}... \t", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}
