#![no_main]
#![no_std]
#![feature(array_windows, array_chunks)]
#![allow(unaligned_references)]

use core::panic::PanicInfo;
use core::fmt::Write;
use machine::memory::MemMap;
use machine::keyboard::{KeyCode, KeyDirection, KeyModifiers};
use sound::{WavFile, Sound, Sample, ActionOnEnd};
use machine::cmos;
use machine;
use event_hook;
use event_hook::{EventKind, Event, box_fn};
use physics::{Rectangle, Point, Object, Velocity};
use num::{Integer, Float};
use sync::mutex::MutexGuard;
use collections::vec::Vec;
use collections::vec;
use artist::{println, print, SCREEN_HEIGHT, SCREEN_WIDTH, Artist, Color, X_SCALE, Y_SCALE};
use artist::bitmap::{Bitmap, ScaledBitmap, Transparency};
use artist;
use collections::allocator::get_allocator;
use lazy_static::{lazy_static, Deref};


sound::sound!(MUSIC, RAW_MUSIC => "./assets/canon-in-d-major.wav", size => 7287938);
sound::sound!(DRUM, RAW_DRUM => "./assets/drum.wav", size => 734028);

pub fn game_entry_point() -> ! {
    println!("Loading...");
    sound::play_sound(MUSIC.deref(), ActionOnEnd::Replay);
    
    loop {
        //let mut panic_writer = PanicWriter { x_pos: 0, y_pos: 0 };
        let mut game = Game::init();
        // The artist is locked by the game at this point
        // Do not use any print macro here until the game hase been dropped
        game.main_loop();
        core::mem::drop(game);
        sound::play_sound(MUSIC.deref(), ActionOnEnd::Replay);
        let mut restart = false;
        let restart_exit_hook = event_hook::hook_event(EventKind::Keyboard, box_fn!(|event| {
            if let Event::Keyboard(keycode, direction, modifiers) = event {
                match keycode {
                    KeyCode::Y => {
                        restart = true;
                    }
                    KeyCode::Escape => {
                        if unsafe { machine::power::shutdown() }.is_err() {
                            println!("Shutdown your computer yourself");
                        }
                    }
                    _ => ()
                }
            }
        }));
        loop {
            if restart {
                event_hook::unhook_event(restart_exit_hook, EventKind::Keyboard);
                break;
            }
        }
    }
}

struct Game {
    ball_char: Character,
    paddle_char: Character,
    has_started: bool,
    paused: bool,
    shutdown_attempted: bool,
    paused_msg_has_been_drawn: bool,
    background: Color,
    blocks: Vec<'static, Character>,
    artist: MutexGuard<'static, Artist>
}

impl Game {
    fn init() -> Self {
        let ball_bmp_bytes = include_bytes!("./assets/ball.bmp");
        let ball_bmp = Bitmap::from(ball_bmp_bytes, Transparency::Black)
            .expect("Failed to read the bitmap from the given source");
        let paddle_bmp_bytes = include_bytes!("./assets/paddle.bmp");
        let paddle_bmp = Bitmap::from(paddle_bmp_bytes, Transparency::Black)
            .expect("Failed to read the bitmap from the given source");
        let paddle_char = Character::new(Object {
                pos: Point(
                    (SCREEN_WIDTH / 2 - paddle_bmp.scaled_width() / 2).as_i16(),
                    (SCREEN_HEIGHT - 20 - paddle_bmp.scaled_height()).as_i16()
                ),
                velocity: Velocity { direction: 0, speed: 0 }
            }, paddle_bmp.convert_to_scaled_bitmap()
        );
        let ball_char = Character::new(Object {
                pos: Point(
                    (SCREEN_WIDTH / 2 - ball_bmp.width() / 2).as_i16(),
                    paddle_char.object.pos.y() - ball_bmp.scaled_height().as_i16()
                ),
                velocity: Velocity { direction: 0, speed: 0 }
            }, ball_bmp.convert_to_scaled_bitmap()
        );
        Self {
            ball_char,
            paddle_char,
            has_started: false,
            paused: false,
            shutdown_attempted: false,
            paused_msg_has_been_drawn: false,
            background: Color::new(Color::Purple),
            blocks: Self::generate_blocks(),
            artist: artist::get_artist().lock()
        }
    }

    fn main_loop(&mut self) {
        let game_hook = event_hook::hook_event(EventKind::Keyboard, box_fn!(|event| {
            if let Event::Keyboard(keycode, direction, modifiers) = event {
                if direction == KeyDirection::Down {
                    match keycode {
                        KeyCode::ArrowRight => {
                            if self.has_started && direction == KeyDirection::Down {
                                if !paddle_collided_with_right_wall(&self.paddle_char) {
                                    self.move_paddle_in_double_buffer(PaddleDirection::Right);
                                }
                            }
                        }
                        KeyCode::ArrowLeft => {
                            if self.has_started && direction == KeyDirection::Down {
                                if !paddle_collided_with_left_wall(&self.paddle_char) {
                                    self.move_paddle_in_double_buffer(PaddleDirection::Left);
                                }
                            }
                        }
                        KeyCode::Enter => {
                            if !self.has_started {
                                self.ball_char.object.velocity.direction = self.generate_direction();
                                self.ball_char.object.velocity.speed = 5;
                                self.has_started = true;
                                sound::play_sound(DRUM.deref(), ActionOnEnd::Replay);
                            } else if self.paused {
                                self.paused = false;
                                self.paused_msg_has_been_drawn = false;
                                sound::play_sound(DRUM.deref(), ActionOnEnd::Replay);
                            }
                            self.shutdown_attempted = false;
                        }
                        KeyCode::Escape => {
                            self.paused = true;
                            sound::play_sound(MUSIC.deref(), ActionOnEnd::Replay);
                        }
                        KeyCode::X => {
                            if self.paused {
                                if unsafe { machine::power::shutdown() }.is_err() {
                                    self.shutdown_attempted = true;
                                }
                            }
                        }
                        _ => ()
                    };
                }
            }
        }));
        self.artist.draw_background_in_double_buffer(&self.background);
        self.draw_game_in_double_buffer();
        self.artist.draw_on_screen_from_double_buffer();
        self.artist.reset_writing_pos();
        loop {
            if !self.has_started && !self.paused {
                self.artist.write_str("Press enter to start\n");
                self.artist.reset_writing_pos();
                continue;
            }
            if self.paused {
                if self.shutdown_attempted {
                    self.artist.write_str("Shut down your computer yourself");
                    self.artist.reset_writing_pos();
                } else {
                    if !self.paused_msg_has_been_drawn {
                        self.draw_game_in_double_buffer();
                        self.artist.draw_on_screen_from_double_buffer();
                        self.artist.write_str("Paused\n");
                        self.artist.write_str("Press enter to continue\n");
                        self.artist.write_str("Press x to exit\n");
                        self.artist.reset_writing_pos();
                        self.paused_msg_has_been_drawn = true
                    }
                }
                continue;
            }
            if self.blocks.len() == 0 {
                self.artist.write_str("You win\n");
                self.artist.write_str("Press y to play again\n");
                self.artist.write_str("Press esc to exit\n");
                self.artist.reset_writing_pos();
                break;
            }
            if ball_collided_with_left_wall(&self.ball_char) {
                // Need to consider the scenario where the direction is 180/0 degrees
                self.ball_char.object.velocity.reflect_about_y_axis();
            } else if ball_collided_with_right_wall(&self.ball_char) {
                // Need to consider the scenario where the direction is 180/0 degrees
                self.ball_char.object.velocity.reflect_about_y_axis();
            } else if ball_collided_with_ceiling(&self.ball_char) {
                // Need to consider the scenario where the direction is 270/90 degrees
                self.ball_char.object.velocity.reflect_about_x_axis();
            } else if self.ball_char.collided_with(&self.paddle_char).0 {
                // Need to consider the scenario where the direction is 270/90 degrees
                self.ball_char.object.velocity.reflect_about_x_axis();
            } else if ball_is_off_screen(&self.ball_char) {
                use core::fmt::Write;
                self.artist.write_str("Game over\n");
                self.artist.write_str("Press y to play again\n");
                self.artist.write_str("Press esc to exit");
                break;
            }
            for i in 0..self.blocks.len() {
                let block_char = &self.blocks[i];
                if self.ball_char.collided_with(block_char).0 {
                    self.artist.erase_scaled_bitmap_from_double_buffer(&block_char.repr, block_char.object.pos, &self.background);
                    self.ball_char.object.velocity.reflect_about_x_axis();
                    self.blocks.remove(i);
                    break;
                } else {
                    //self.artist.draw_scaled_bitmap_in_double_buffer(block_char.object.pos, &block_char.repr);
                }
            }
            let old_pos = self.ball_char.object.update_pos(1, X_SCALE, Y_SCALE);
            let (ball_passed_through_paddle, point_at_paddle_level_opt) = ball_passed_through_paddle(old_pos, self.ball_char.object.pos, self.ball_char.object.velocity.direction, &self.paddle_char);
            if ball_passed_through_paddle {
                self.ball_char.object.pos = point_at_paddle_level_opt.unwrap();
            }
            self.artist.move_scaled_bitmap_in_double_buffer(&self.ball_char.repr, old_pos, self.ball_char.object.pos, &self.background);
            self.draw_game_in_double_buffer();
            self.artist.draw_on_screen_from_double_buffer();
        }
        //core::mem::drop(artist);
        event_hook::unhook_event(game_hook, EventKind::Keyboard);
    }

    fn move_paddle_in_double_buffer(&mut self, direction: PaddleDirection) {
        let diff = match direction {
            PaddleDirection::Left => Point(-4 * X_SCALE.as_i16(), 0),
            PaddleDirection::Right => Point(4 * X_SCALE.as_i16(), 0)
        };
        let old_pos = self.paddle_char.object.pos;
        self.paddle_char.object.pos += diff;
        self.artist.move_scaled_bitmap_in_double_buffer(&self.paddle_char.repr, old_pos, self.paddle_char.object.pos, &self.background);
    }

    fn generate_blocks() -> Vec<'static, Character> {
        let blue_block_bmp_bytes = include_bytes!("./assets/blue_block.bmp");
        let blue_block_bmp = Bitmap::from(blue_block_bmp_bytes, Transparency::None)
            .expect("Failed to read the bitmap from the given source");
        let cyan_block_bmp_bytes = include_bytes!("./assets/cyan_block.bmp");
        let cyan_block_bmp = Bitmap::from(cyan_block_bmp_bytes, Transparency::None)
            .expect("Failed to read the bitmap from the given source");
        let green_block_bmp_bytes = include_bytes!("./assets/green_block.bmp");
        let green_block_bmp = Bitmap::from(green_block_bmp_bytes, Transparency::None)
            .expect("Failed to read the bitmap from the given source");
        let pink_block_bmp_bytes = include_bytes!("./assets/pink_block.bmp");
        let pink_block_bmp = Bitmap::from(pink_block_bmp_bytes, Transparency::None)
            .expect("Failed to read the bitmap from the given source");
        let yellow_block_bmp_bytes = include_bytes!("./assets/yellow_block.bmp");
        let yellow_block_bmp = Bitmap::from(yellow_block_bmp_bytes, Transparency::None)
            .expect("Failed to read the bitmap from the given source");
        let block_bmps = [blue_block_bmp, pink_block_bmp, green_block_bmp, cyan_block_bmp, yellow_block_bmp];
        let mut blocks = vec!(item_type => Character, capacity => 10);
        let BLOCK_START_POS_X: usize = 15;
        let BLOCK_END_POS_X: usize = (SCREEN_WIDTH - BLOCK_START_POS_X - block_bmps[0].scaled_width());
        let BLOCK_START_POS_Y: usize = 10;
        let BLOCK_END_POS_Y: usize = SCREEN_HEIGHT / 4;
        let mut i = 0;
        for y in (BLOCK_START_POS_Y..=BLOCK_END_POS_Y).step_by(block_bmps[0].scaled_height()) {
            for x in (BLOCK_START_POS_X..=BLOCK_END_POS_X).step_by(block_bmps[0].scaled_width()) {
                let block = Character::new(Object {
                    pos: Point(x.as_i16(), y.as_i16()),
                    velocity: Velocity { direction: 0, speed: 0 }
                }, block_bmps[i].convert_to_scaled_bitmap());
                blocks.push(block);
                i = (i + 1) % block_bmps.len();
            }
        }
        blocks
    }

    /// Returns an angle in degrees that can be used for an initial angle
    /// for the ball movement in the game
    fn generate_direction(&self) -> usize {
        return 220;
        /*
        // The current time is random enough for this purpose
        let time = cmos::get_current_time();
        // Adding 180 because the initial direction can't be anything
        // lesser than 180.
        // Anything lesser than 180 will result in the ball moving downwards
        let direction = (time.sum_of_fields() % 180) + 180;
        // A direction of 180 will result in weird movements to the left only
        if direction == 180 {
            direction + 10
        // A direction of 270 will result in weird movements to the right only
        } else if direction == 270 {
            direction + 10
        } else {
            direction
        }
        */
    }

    fn draw_game_in_double_buffer(&mut self) {
        self.artist.draw_scaled_bitmap_in_double_buffer(self.paddle_char.object.pos, &self.paddle_char.repr);
        for i in 0..self.blocks.len() {
            self.artist.draw_scaled_bitmap_in_double_buffer(self.blocks[i].object.pos, &self.blocks[i].repr);
        }
        self.artist.draw_scaled_bitmap_in_double_buffer(self.ball_char.object.pos, &self.ball_char.repr);
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
    ball_char.object.pos.x() >= SCREEN_WIDTH as i16 - ball_char.repr.width().as_i16()
}

fn ball_collided_with_ceiling(ball_char: &Character) -> bool {
    ball_char.object.pos.y() <= 0 + 5
}

fn ball_collided_with_paddle(ball_char: &Character, paddle_char: &Character) -> bool {
    ball_char.object.pos.y() >= paddle_char.object.pos.y()
        && ball_char.object.pos.x() >= paddle_char.object.pos.x()
        && ball_char.object.pos.x() <= paddle_char.object.pos.x() + paddle_char.repr.width().as_i16()
}

fn ball_is_off_screen(ball_char: &Character) -> bool {
    ball_char.object.pos.y() >= SCREEN_HEIGHT.as_i16()
}

fn paddle_collided_with_right_wall(paddle_char: &Character) -> bool {
    paddle_char.object.pos.x() + paddle_char.repr.width().as_i16() >= SCREEN_WIDTH.as_i16() - 8
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
        y_distance_between_pos_and_paddle_level * direction_of_ball_from_paddle_perspective.cosf32().as_i16()
    ) / direction_of_ball_from_paddle_perspective.sinf32().as_i16();
    let ball_x_pos_at_paddle_level = old_pos.x() - distance_between_x_pos_at_paddle_level_and_old_pos;
    let ball_passed_through_paddle = ball_x_pos_at_paddle_level >= paddle_char.object.pos.x()
        && ball_x_pos_at_paddle_level <= paddle_char.object.pos.x() + paddle_char.repr.width().as_i16() - 1;
    let point_at_which_ball_passed_through_paddle_level = Point(ball_x_pos_at_paddle_level, paddle_char.object.pos.y());
    (ball_passed_through_paddle, Some(point_at_which_ball_passed_through_paddle_level))
}

/// Anything with physical properties that can be drawn
#[derive(Clone)]
struct Character {
    /// The physical definition of the character?
    object: Object,
    repr: ScaledBitmap,
    visibility: Visibility
}

impl Character {
    /// Creates a new character with a default visibility of visible
    fn new(object: Object, repr: ScaledBitmap) -> Self {
        Self {
            object,
            repr,
            visibility: Visibility::Visible
        }
    }

    fn collided_with(&self, other_char: &Character) -> (bool, CollidedFrom) {
        let collided = 
        self.object.pos.y() >= other_char.object.pos.y()
            && self.object.pos.y() <= other_char.object.pos.y() + other_char.repr.height().as_i16()
            && self.object.pos.x() >= other_char.object.pos.x()
            && self.object.pos.x() <= other_char.object.pos.x() + other_char.repr.width().as_i16();
        let collided_from = match self.object.velocity.direction {
            0..=180 => CollidedFrom::Bottom,
            181..=360 => CollidedFrom::Top,
            _ => unreachable!()
        };
        (collided, collided_from)
    }

    
}

/// Tells from which direction a collision occured
enum CollidedFrom {
    Top,
    Bottom
}

/// Tells whether or not a character should be shown on the screen
#[derive(Clone, PartialEq)]
enum Visibility {
    Visible,
    Invisible
}
