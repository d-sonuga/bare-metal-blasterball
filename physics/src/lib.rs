//! Abstractions dealing with the laws of physics in the game world

#![cfg_attr(not(test), no_std)]

use core::ops::{Add, Sub, AddAssign, SubAssign};
use num::{Integer, Float};

#[derive(Clone)]
pub struct Object {
    /// The top left point of the object on the screen
    pub pos: Point,
    pub velocity: Velocity
}

impl Object {
    pub fn update_pos(&mut self, time: usize, x_scale: usize, y_scale: usize) -> Point {
        let dx = self.velocity.horizontal_component() * time as i16;
        let dy = self.velocity.vertical_component() * time as i16;
        let old_pos = self.pos;
        self.pos += Point(dx * x_scale.as_i16(), dy * y_scale.as_i16());
        old_pos
    }
}

#[derive(Clone)]
pub struct Velocity {
    /// The angle in a circular coordinate system, assuming the center is `pos`
    pub direction: usize,
    pub speed: usize
}

impl Velocity {
    #[inline]
    pub fn horizontal_component(&self) -> i16 {
        self.speed as i16 * self.direction.cosf32().as_i16()
    }
    #[inline]
    pub fn vertical_component(&self) -> i16 {
        self.speed as i16 * self.direction.sinf32().as_i16()
    }
    #[inline]
    pub fn reflect_about_y_axis(&mut self) {
        match self.direction {
            0..=180 => self.direction = 180 - self.direction,
            181..=360 => self.direction = 540 - self.direction,
            _ => unreachable!()
        }
    }
    #[inline]
    pub fn reflect_about_x_axis(&mut self) {
        self.direction = 360 - self.direction;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
pub struct Point(pub i16, pub i16);

impl Point {
    #[inline]
    pub fn x(&self) -> i16 {
        self.0
    }
    #[inline]
    pub fn y(&self) -> i16 {
        self.1 as i16
    }
}

impl Add for Point {
    type Output = Point;
    #[inline]
    fn add(self, rhs: Point) -> Self::Output {
        Point(self.0 + rhs.0, self.1 + rhs.1)
    }
}

impl Sub for Point {
    type Output = Point;
    #[inline]
    fn sub(self, rhs: Point) -> Self::Output {
        Point(self.0 - rhs.0, self.1 - rhs.1)
    }
}

impl AddAssign for Point {
    #[inline]
    fn add_assign(&mut self, rhs: Point) {
        self.0 += rhs.0;
        self.1 += rhs.1;
    }
}

impl SubAssign for Point {
    #[inline]
    fn sub_assign(&mut self, rhs: Point) {
        self.0 -= rhs.0;
        self.1 -= rhs.1;
    }
}

pub struct Rectangle {
    pub top_left: Point,
    pub width: usize,
    pub height: usize
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_pos() {
        let mut object = Object {
            pos: Point(0, 0),
            velocity: Velocity {
                direction: 0,
                speed: 1
            }
        };
        let old_pos = object.update_pos(1);
        assert_eq!(old_pos, Point(0, 0));
        assert_eq!(object.pos, Point(1, 0));

        let mut object = Object {
            pos: Point(0, 0),
            velocity: Velocity {
                direction: 270,
                speed: 1
            }
        };
        object.update_pos(1);
        assert_eq!(object.pos, Point(0, -1));

        let mut object = Object {
            pos: Point(5, 6),
            velocity: Velocity {
                direction: 270,
                speed: 1
            }
        };
        object.update_pos(1);
        assert_eq!(object.pos, Point(5, 5));

        let mut object = Object {
            pos: Point(5, 6),
            velocity: Velocity {
                direction: 180,
                speed: 1
            }
        };
        object.update_pos(1);
        assert_eq!(object.pos, Point(4, 6));

        let mut object = Object {
            pos: Point(0, 0),
            velocity: Velocity {
                direction: 270,
                speed: 1
            }
        };
        object.update_pos(1);
        assert_eq!(object.pos, Point(0, -1));
    }

    #[test]
    fn point_arithmetic() {
        let mut x = Point(3, 3);
        let y = Point(2, 2);
        x += y;
        assert_eq!(x, Point(5, 5));

        let x = Point(0, 32);
        let y = Point(2, 33);
        assert_eq!(x - y, Point(-2, -1));

        let x = Point(43, 1);
        let y = Point(1, -19);
        assert_eq!(x + y, Point(44, -18));

        let mut x = Point(43, 22);
        let y = Point(43, 21);
        x -= y;
        assert_eq!(x, Point(0, 1));
    }
}