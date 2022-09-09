#![no_main]
#![no_std]

use core::panic::PanicInfo;
use machine::memory::MemMap;
use event_hook;
use drivers::keyboard::{KeyCode, KeyDirection, KeyModifiers};
use physics::{Rectangle, Point, Object, Velocity};
use num::{Integer, Float};
use sync::mutex::MutexGuard;
use artist::{println, print, SCREEN_HEIGHT, SCREEN_WIDTH, Artist, MoveBitmapInDoubleBufferRequest};
use artist::bitmap::Bitmap;
use artist;


#[no_mangle]
pub extern "C" fn entry_point(mmap: MemMap) -> ! {
    let mut game = Game::init();
    game.main_loop();
    loop {}
}

struct Game {
    ball_char: Character,
    paddle_char: Character,
    has_started: bool,
    artist: MutexGuard<'static, Artist>,
    background: Bitmap
}

impl Game {
    fn init() -> Self {
        let background_bmp_bytes = include_bytes!("./assets/background.bmp");
        let background_bmp = Bitmap::from(background_bmp_bytes)
            .expect("Failed to read the bitmap from the given source");
        let ball_bmp_bytes = include_bytes!("./assets/ball.bmp");
        let ball_bmp = Bitmap::from(ball_bmp_bytes)
            .expect("Failed to read the bitmap from the given source");
        let paddle_bmp_bytes = include_bytes!("./assets/paddle.bmp");
        let paddle_bmp = Bitmap::from(paddle_bmp_bytes)
            .expect("Failed to read the bitmap from the given source");
        let paddle_char = Character {
            object: Object {
                pos: Point(160 - (paddle_bmp.width() / 2) as i16 - 8, 200 - 8 - paddle_bmp.height() as i16),
                velocity: Velocity { direction: 0, speed: 0 }
            },
            repr: paddle_bmp
        };
        let ball_char = Character {
            object: Object {
                pos: paddle_char.object.pos - Point(0, 14) + Point(18, 0),
                velocity: Velocity { direction: 0, speed: 0 }
            },
            repr: ball_bmp
        };
        Self {
            ball_char,
            paddle_char,
            has_started: false,
            artist: artist::get_artist().lock(),
            background: background_bmp
        }
    }

    fn main_loop(&mut self) {
        self.artist.draw_bitmap_in_double_buffer(Point(0, 0), &self.background);
        self.artist.draw_bitmap_in_double_buffer(self.ball_char.object.pos, &self.ball_char.repr);
        self.artist.draw_bitmap_in_double_buffer(self.paddle_char.object.pos, &self.paddle_char.repr);
        self.artist.redraw_on_screen_from_double_buffer();
        event_hook::hook_event(event_hook::Event::keyboard(), event_hook::box_fn!(|event| {
            if let event_hook::Event::Keyboard(keycode, direction, modifiers) = event {
                match keycode {
                    KeyCode::ArrowRight => {
                        if self.has_started && direction == KeyDirection::Down {
                            if !paddle_collided_with_right_wall(&self.paddle_char) {
                                self.move_paddle(PaddleDirection::Right);
                            }
                        }
                    }
                    KeyCode::ArrowLeft => {
                        if self.has_started && direction == KeyDirection::Down {
                            if !paddle_collided_with_left_wall(&self.paddle_char) {
                                self.move_paddle(PaddleDirection::Left);
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if !self.has_started {
                            self.ball_char.object.velocity.direction = 210;
                            self.ball_char.object.velocity.speed = 5;
                            self.has_started = true;
                        }
                    }
                    _ => ()
                };
            }
        }));
        loop {
            if ball_collided_with_left_wall(&self.ball_char) {
                self.ball_char.object.velocity.reflect_about_y_axis();
            } else if ball_collided_with_right_wall(&self.ball_char) {
                self.ball_char.object.velocity.reflect_about_y_axis();
            } else if ball_collided_with_ceiling(&self.ball_char) {
                self.ball_char.object.velocity.reflect_about_x_axis();
            } else if ball_collided_with_paddle(&self.ball_char, &self.paddle_char) {
                self.ball_char.object.velocity.reflect_about_x_axis();
            } else if ball_is_off_screen(&self.ball_char) {
                use core::fmt::Write;
                self.artist.write_str("Game over");
                break;
            }
            let old_pos = self.ball_char.object.update_pos(1);
            let (ball_passed_through_paddle, point_at_paddle_level_opt) = ball_passed_through_paddle(old_pos, self.ball_char.object.pos, self.ball_char.object.velocity.direction, &self.paddle_char);
            if ball_passed_through_paddle {
                self.ball_char.object.pos = point_at_paddle_level_opt.unwrap();
            }
            self.artist.draw_background_in_double_buffer(&self.background);
            self.artist.draw_bitmap_in_double_buffer(self.paddle_char.object.pos, &self.paddle_char.repr);
            self.artist.draw_bitmap_in_double_buffer(self.ball_char.object.pos, &self.ball_char.repr);
            self.artist.redraw_on_screen_from_double_buffer();
        }
    }

    fn move_paddle(&mut self, direction: PaddleDirection) {
        let diff = match direction {
            PaddleDirection::Left => Point(-4, 0),
            PaddleDirection::Right => Point(4, 0)
        };
        let old_pos = self.paddle_char.object.pos;
        self.paddle_char.object.pos += diff;
    }
}

/// The direction to move a paddle in
enum PaddleDirection {
    Left,
    Right
}

fn ball_collided_with_left_wall(ball_char: &Character) -> bool {
    ball_char.object.pos.x() <= 0
}

fn ball_collided_with_right_wall(ball_char: &Character) -> bool {
    ball_char.object.pos.x() >= SCREEN_WIDTH as i16 - ball_char.repr.width().to_i16()
}

fn ball_collided_with_ceiling(ball_char: &Character) -> bool {
    ball_char.object.pos.y() <= 0 + 5
}

fn ball_collided_with_paddle(ball_char: &Character, paddle_char: &Character) -> bool {
    ball_char.object.pos.y() >= paddle_char.object.pos.y()
        && ball_char.object.pos.x() >= paddle_char.object.pos.x()
        && ball_char.object.pos.x() <= paddle_char.object.pos.x() + paddle_char.repr.width().to_i16()
}

fn ball_is_off_screen(ball_char: &Character) -> bool {
    ball_char.object.pos.y() >= SCREEN_HEIGHT.to_i16()
}

fn paddle_collided_with_right_wall(paddle_char: &Character) -> bool {
    paddle_char.object.pos.x() + paddle_char.repr.width().to_i16() >= SCREEN_WIDTH.to_i16() - 8
}

fn paddle_collided_with_left_wall(paddle_char: &Character) -> bool {
    paddle_char.object.pos.x() <= 0 + 5
}

fn ball_passed_through_paddle(old_pos: Point, new_pos: Point, direction: usize, paddle_char: &Character) -> (bool, Option<Point>) {
    if new_pos.y() < paddle_char.object.pos.y() {
        return (false, None);
    }
    let direction_of_ball_from_paddle_perspective = 180 + direction;
    let y_distance_between_pos_and_paddle_level = paddle_char.object.pos.y() - old_pos.y();
    let distance_between_x_pos_at_paddle_level_and_old_pos = (
        y_distance_between_pos_and_paddle_level * direction_of_ball_from_paddle_perspective.cosf32().to_i16()
    ) / direction_of_ball_from_paddle_perspective.sinf32().to_i16();
    let ball_x_pos_at_paddle_level = old_pos.x() - distance_between_x_pos_at_paddle_level_and_old_pos;
    let ball_passed_through_paddle = ball_x_pos_at_paddle_level >= paddle_char.object.pos.x()
        && ball_x_pos_at_paddle_level <= paddle_char.object.pos.x() + paddle_char.repr.width().to_i16() - 1;
    let point_at_which_ball_passed_through_paddle_level = Point(ball_x_pos_at_paddle_level, paddle_char.object.pos.y());
    (ball_passed_through_paddle, Some(point_at_which_ball_passed_through_paddle_level))
}

struct Character {
    /// The physical definition of the character?
    object: Object,
    repr: Bitmap
}
