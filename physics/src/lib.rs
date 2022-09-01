//! Abstractions dealing with the laws of physics in the game world

#![no_std]

pub struct Point(pub usize, pub usize);

impl Point {
    pub fn x(&self) -> usize {
        self.0
    }
    pub fn y(&self) -> usize {
        self.1
    }
}

pub struct Rectangle {
    pub top_left: Point,
    pub width: usize,
    pub height: usize
}
