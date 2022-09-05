//! Abstractions dealing with the laws of physics in the game world

#![no_std]

use core::ops::{Add, AddAssign};

pub struct Object {
    /// The top left point of the object on the screen
    pub pos: Point,
    pub velocity: Velocity
}

impl Object {
    pub fn update_pos(&mut self, time: usize) {
        let dx = self.velocity.horizontal_component() * time;
        let dy = self.velocity.vertical_component() * time;
        self.pos += Point(dx, dy);
    }
}

pub struct Velocity {
    /// The angle in a circular coordinate system, assuming the center is `pos`
    pub direction: usize,
    pub speed: usize
}

impl Velocity {
    fn horizontal_component(&self) -> usize {
        //(self.speed * self.direction.cos()).floor() as usize
        1
    }
    fn vertical_component(&self) -> usize {
        //(self.speed * self.direction.sin()).floor() as usize
        1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point(pub usize, pub usize);

impl Point {
    pub fn x(&self) -> usize {
        self.0
    }
    pub fn y(&self) -> usize {
        self.1
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, rhs: Point) {
        self.0 += rhs.0;
        self.1 += rhs.1;
    }
}

pub struct Rectangle {
    pub top_left: Point,
    pub width: usize,
    pub height: usize
}
