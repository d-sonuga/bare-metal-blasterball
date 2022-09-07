#![no_main]
#![no_std]

use core::panic::PanicInfo;
use machine::memory::MemMap;
use event_hook;
use drivers::keyboard::{KeyCode, KeyDirection, KeyModifiers};
use physics::{Rectangle, Point, Object, Velocity};
use artist::{println, print, SCREEN_HEIGHT, SCREEN_WIDTH};
use artist::bitmap::Bitmap;
use artist;


#[no_mangle]
pub extern "C" fn entry_point(mmap: MemMap) -> ! {
    let mut draw = true;
    let ball_bmp_bytes = include_bytes!("./assets/ball.bmp");
    let ball_bmp = Bitmap::from(ball_bmp_bytes)
        .expect("Failed to read the bitmap from the given source");
    let paddle_bmp_bytes = include_bytes!("./assets/paddle.bmp");
    let paddle_bmp = Bitmap::from(paddle_bmp_bytes)
        .expect("Failed to read the bitmap from the given source");
    let mut paddle_char = Character {
        object: Object {
            pos: Point(160 - (paddle_bmp.width() >> 1) as i16 - 8, 200 - 8 - paddle_bmp.height() as i16),
            velocity: Velocity { direction: 0, speed: 0 }
        },
        repr: paddle_bmp
    };
    let mut ball_char = Character {
        object: Object {
            pos: paddle_char.object.pos - Point(0, 14) + Point(18, 0),
            velocity: Velocity { direction: 0, speed: 0 }
        },
        repr: ball_bmp
    };
    let mut artist = artist::get_artist().lock();
    artist.draw_bitmap(ball_char.object.pos, &ball_char.repr);
    artist.draw_bitmap(paddle_char.object.pos, &paddle_char.repr);
    core::mem::drop(artist);

    /*event_hook::hook_event(event_hook::Event::Timer, event_hook::box_fn!(|_| {
        if ball_char.object.pos.x() < 320 && ball_char.object.pos.y() < 200 {
            let old_pos = ball_char.object.update_pos(2);
            let mut artist = artist::get_artist().lock();
            artist.move_bitmap(old_pos, ball_char.object.pos, &ball_char.repr);
        }
    }));*/
    let mut has_started = false;
    event_hook::hook_event(event_hook::Event::keyboard(), event_hook::box_fn!(|event| {
        if let event_hook::Event::Keyboard(keycode, direction, modifiers) = event {
            match keycode {
                KeyCode::ArrowRight => {
                    if direction == KeyDirection::Down {
                        // TODO: Check for collision with right wall
                        paddle_char.object.pos += Point(1, 0)
                    }
                }
                KeyCode::ArrowLeft => {
                    if direction == KeyDirection::Down {
                        // TODO: Check for collision with left wall
                        paddle_char.object.pos += Point(-1, 0);
                    }
                }
                KeyCode::Enter => {
                    if !has_started {
                        ball_char.object.velocity.direction = 210;
                        ball_char.object.velocity.speed = 1;
                        has_started = true;
                    }
                }
                _ => ()
            };
        }
    }));
    loop {
        if collided_with_left_wall(&ball_char) {
            ball_char.object.velocity.reflect_about_y_axis();
        } else if collided_with_right_wall(&ball_char) {
            ball_char.object.velocity.reflect_about_y_axis();
        } else if collided_with_ceiling(&ball_char) {
            ball_char.object.velocity.reflect_about_x_axis();
        } //else if collided_with_paddle()
        let old_pos = ball_char.object.update_pos(1);
        let mut artist = artist::get_artist().lock();
        artist.move_bitmap(old_pos, ball_char.object.pos, &ball_char.repr);
    }
}

struct Game {
    ball_char: Character,
    paddle_char: Character
}

impl Game {
    fn init() -> Self {
        let ball_bmp_bytes = include_bytes!("./assets/ball.bmp");
        let ball_bmp = Bitmap::from(ball_bmp_bytes)
            .expect("Failed to read the bitmap from the given source");
        let paddle_bmp_bytes = include_bytes!("./assets/paddle.bmp");
        let paddle_bmp = Bitmap::from(paddle_bmp_bytes)
            .expect("Failed to read the bitmap from the given source");
        Self {
            ball_char: Character {
                object: Object {
                    pos: paddle_char.object.pos - Point(0, 14) + Point(18, 0),
                    velocity: Velocity { direction: 0, speed: 0 }
                },
                repr: ball_bmp
            },
            paddle_char: Character {
                object: Object {
                    pos: Point(160 - (paddle_bmp.width() >> 1) as i16 - 8, 200 - 8 - paddle_bmp.height() as i16),
                    velocity: Velocity { direction: 0, speed: 0 }
                },
                repr: paddle_bmp
            }
        }
    }

    fn main_loop() -> ! {
        loop {
            
        }
    }
}

fn collided_with_left_wall(ball_char: &Character) -> bool {
    ball_char.object.pos.x() <= 8
}

fn collided_with_right_wall(ball_char: &Character) -> bool {
    ball_char.object.pos.x() >= SCREEN_WIDTH as i16 - 1 - 8
}

fn collided_with_ceiling(ball_char: &Character) -> bool {
    ball_char.object.pos.y() <= 8
}

struct Character {
    /// The physical definition of the character?
    object: Object,
    repr: Bitmap
}
