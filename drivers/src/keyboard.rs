//! PS/2 Keyboard driver for US 104 layout


/// The beginning byte for an extended key code
const EXTENDED_KEY_CODE: u8 = 0xe0;

/// A representation of the state of the keyboard
pub struct Keyboard {
    /// Tells whether the last processed byte was the beginning of an extended key code
    /// on a regular or extended key
    state: KeyboardState,
    /// Tells whether or not shift, ctrl, alt,... is down
    modifiers: KeyModifiers
}

/// For toggling modifier states
macro_rules! toggle_modifier {
    ($self:ident . $mod:ident, $direction:ident) => {
        match $direction {
            KeyDirection::Down => $self.modifiers.$mod = true,
            KeyDirection::Up => $self.modifiers.$mod = false
        }
    }
}

impl Keyboard {
    /// Creates a new instance of Keyboard
    pub fn new() -> Self {
        Keyboard {
            state: KeyboardState::Start,
            modifiers: KeyModifiers::new()
        }
    }

    /// Accepts a byte and changes the keyboard state in the case of beginning or end of an extended code.
    /// Else, just returns the event associated with the scancode byte.
    ///
    /// # Example
    ///
    /// ```
    /// let mut kbd = Keyboard::new();
    ///
    /// // Beginning of an extended code
    /// let event = kbd.process_byte(0xe0);
    /// assert_eq!(event, Ok(None));
    ///
    /// // End of an extended code
    /// let event = kbd.process_byte(0x10);
    /// assert_eq!(event, Ok(Some(KeyEvent {
    ///        keycode: KeyCode::PrevTrack
    ///        key_modifiers: KeyModifiers::new(),
    ///        direction: KeyDirection::Down
    /// })));
    ///
    /// // Regular event
    /// let event = kbd.process_byte(0x01);
    /// assert_eq!(event, Ok(Some(KeyEvent {
    ///        keycode: KeyCode::Escape,
    ///        key_modifiers: KeyModifiers::new(),
    ///        direction: KeyDirection::Down
    /// })));
    /// ```
    pub fn process_byte(&mut self, byte: u8) -> Result<Option<KeyEvent>, KeyError> {
        match self.state {
            KeyboardState::Start => {
                match byte {
                    // The beginning of an extended key press
                    EXTENDED_KEY_CODE => {
                        self.state = KeyboardState::Extended;
                        Ok(None)
                    }
                    // The range of scan codes for regular key presses
                    0x01..=0x58 => {
                        let keycode = self.map_scancode(byte)?;
                        if keycode.is_modifier() {
                            self.transition_modifier(keycode, KeyDirection::Down);
                            Ok(None)
                        } else {
                            Ok(Some(KeyEvent {
                                keycode,
                                key_modifiers: self.modifiers,
                                direction: KeyDirection::Down
                            }))
                        }
                    }
                    // For key releases
                    0x81..=0xd8 => {
                        let keycode = self.map_scancode(byte - 0x80)?;
                        if keycode.is_modifier() {
                            self.transition_modifier(keycode, KeyDirection::Up);
                            Ok(None)
                        } else {
                            Ok(Some(KeyEvent {
                                keycode,
                                key_modifiers: self.modifiers,
                                direction: KeyDirection::Up
                            }))
                        }
                    }
                    _ => Err(KeyError::UnknownScancode)
                }
            }
            KeyboardState::Extended => {
                // Reset keyboard state
                self.state = KeyboardState::Start;
                match byte {
                    // Range of scancodes for extended key presses
                    0x10..=0x90 => {
                        let keycode = self.map_extended_scancode(byte)?;
                        if keycode.is_modifier() {
                            self.transition_modifier(keycode, KeyDirection::Down);
                            Ok(None)
                        } else {
                            Ok(Some(KeyEvent {
                                keycode,
                                key_modifiers: self.modifiers,
                                direction: KeyDirection::Down
                            }))
                        }
                    }
                    // Range for extended key releases
                    0x99..=0xed => {
                        let keycode = self.map_extended_scancode(byte - 0x80)?;
                        if keycode.is_modifier() {
                            self.transition_modifier(keycode, KeyDirection::Up);
                            Ok(None)
                        } else {
                            Ok(Some(KeyEvent {
                                keycode,
                                key_modifiers: self.modifiers,
                                direction: KeyDirection::Up
                            }))
                        }
                    }
                    _ => Err(KeyError::UnknownScancode)
                }
            }
        }
    }

    fn transition_modifier(&mut self, keycode: KeyCode, direction: KeyDirection) {
        match keycode {
            KeyCode::LeftCtrl => toggle_modifier!(self.lctrl, direction),
            KeyCode::RightCtrl => toggle_modifier!(self.rctrl, direction),
            KeyCode::LeftShift => toggle_modifier!(self.lshift, direction),
            KeyCode::RightShift => toggle_modifier!(self.rshift, direction),
            KeyCode::LeftAlt => toggle_modifier!(self.alt, direction),
            KeyCode::AltGr => toggle_modifier!(self.alt_gr, direction),
            KeyCode::CapsLock => toggle_modifier!(self.caps_lock, direction),
            kc => panic!("Not a modifier: {:?}", kc)
        }
    }

    fn map_scancode(&self, byte: u8) -> Result<KeyCode, KeyError> {
        match byte {
            0x01 => Ok(KeyCode::Escape),
            0x02 => Ok(KeyCode::One),
            0x03 => Ok(KeyCode::Two),
            0x04 => Ok(KeyCode::Three),
            0x05 => Ok(KeyCode::Four),
            0x06 => Ok(KeyCode::Five),
            0x07 => Ok(KeyCode::Six),
            0x08 => Ok(KeyCode::Seven),
            0x09 => Ok(KeyCode::Eight),
            0x0a => Ok(KeyCode::Nine),
            0x0b => Ok(KeyCode::Zero),
            0x0c => Ok(KeyCode::Dash),
            0x0d => Ok(KeyCode::Equals),
            0x0e => Ok(KeyCode::Backspace),
            0x0f => Ok(KeyCode::Tab),
            0x10 => Ok(KeyCode::Q),
            0x11 => Ok(KeyCode::W),
            0x12 => Ok(KeyCode::E),
            0x13 => Ok(KeyCode::R),
            0x14 => Ok(KeyCode::T),
            0x15 => Ok(KeyCode::Y),
            0x16 => Ok(KeyCode::U),
            0x17 => Ok(KeyCode::I),
            0x18 => Ok(KeyCode::O),
            0x19 => Ok(KeyCode::P),
            0x1a => Ok(KeyCode::OpenBracket),
            0x1b => Ok(KeyCode::CloseBracket),
            0x1c => Ok(KeyCode::Enter),
            0x1d => Ok(KeyCode::LeftCtrl),
            0x1e => Ok(KeyCode::A),
            0x1f => Ok(KeyCode::S),
            0x20 => Ok(KeyCode::D),
            0x21 => Ok(KeyCode::F),
            0x22 => Ok(KeyCode::G),
            0x23 => Ok(KeyCode::H),
            0x24 => Ok(KeyCode::J),
            0x25 => Ok(KeyCode::K),
            0x26 => Ok(KeyCode::L),
            0x27 => Ok(KeyCode::SemiColon),
            0x28 => Ok(KeyCode::SingleQuote),
            0x29 => Ok(KeyCode::Backtick),
            0x2a => Ok(KeyCode::LeftShift),
            0x2b => Ok(KeyCode::BackSlash),
            0x2c => Ok(KeyCode::Z),
            0x2d => Ok(KeyCode::X),
            0x2e => Ok(KeyCode::C),
            0x2f => Ok(KeyCode::V),
            0x30 => Ok(KeyCode::B),
            0x31 => Ok(KeyCode::N),
            0x32 => Ok(KeyCode::M),
            0x33 => Ok(KeyCode::Comma),
            0x34 => Ok(KeyCode::Dot),
            0x35 => Ok(KeyCode::ForwardSlash),
            0x36 => Ok(KeyCode::RightShift),
            0x37 => Ok(KeyCode::KeypadStar),
            0x38 => Ok(KeyCode::LeftAlt),
            0x39 => Ok(KeyCode::Space),
            0x3a => Ok(KeyCode::CapsLock),
            0x3b => Ok(KeyCode::F1),
            0x3c => Ok(KeyCode::F2),
            0x3d => Ok(KeyCode::F3),
            0x3e => Ok(KeyCode::F4),
            0x3f => Ok(KeyCode::F5),
            0x40 => Ok(KeyCode::F6),
            0x41 => Ok(KeyCode::F7),
            0x42 => Ok(KeyCode::F8),
            0x43 => Ok(KeyCode::F9),
            0x44 => Ok(KeyCode::F10),
            0x57 => Ok(KeyCode::F11),
            0x58 => Ok(KeyCode::F12),
            0x45 => Ok(KeyCode::NumLock),
            0x46 => Ok(KeyCode::ScrollLock),
            0x47 => Ok(KeyCode::KeypadSeven),
            0x48 => Ok(KeyCode::KeypadEight),
            0x49 => Ok(KeyCode::KeypadNine),
            0x4a => Ok(KeyCode::KeypadDash),
            0x4b => Ok(KeyCode::KeypadFour),
            0x4c => Ok(KeyCode::KeypadFive),
            0x4d => Ok(KeyCode::KeypadSix),
            0x4e => Ok(KeyCode::KeypadPlus),
            0x4f => Ok(KeyCode::KeypadOne),
            0x50 => Ok(KeyCode::KeypadTwo),
            0x51 => Ok(KeyCode::KeypadThree),
            0x52 => Ok(KeyCode::KeypadZero),
            0x53 => Ok(KeyCode::KeypadDot),
            _ => Err(KeyError::UnknownScancode)
        }
    }

    fn map_extended_scancode(&self, byte: u8) -> Result<KeyCode, KeyError> {
        match byte {
            0x10 => Ok(KeyCode::PrevTrack),
            0x19 => Ok(KeyCode::NextTrack),
            0x1c => Ok(KeyCode::KeypadEnter),
            0x1d => Ok(KeyCode::RightCtrl),
            0x20 => Ok(KeyCode::Mute),
            0x21 => Ok(KeyCode::Calculator),
            0x22 => Ok(KeyCode::Play),
            0x24 => Ok(KeyCode::Stop),
            0x2e => Ok(KeyCode::VolumeDown),
            0x30 => Ok(KeyCode::VolumeUp),
            0x32 => Ok(KeyCode::WWWHome),
            0x35 => Ok(KeyCode::KeypadForwardSlash),
            0x38 => Ok(KeyCode::AltGr),
            0x47 => Ok(KeyCode::Home),
            0x48 => Ok(KeyCode::ArrowUp),
            0x49 => Ok(KeyCode::PageUp),
            0x4b => Ok(KeyCode::ArrowLeft),
            0x4d => Ok(KeyCode::ArrowRight),
            0x4f => Ok(KeyCode::End),
            0x50 => Ok(KeyCode::ArrowDown),
            0x51 => Ok(KeyCode::PageDown),
            0x52 => Ok(KeyCode::Insert),
            0x53 => Ok(KeyCode::Delete),
            0x5b => Ok(KeyCode::LeftGUI),
            0x5c => Ok(KeyCode::RightGUI),
            0x5d => Ok(KeyCode::Apps),
            0x5e => Ok(KeyCode::AcpiPower),
            0x5f => Ok(KeyCode::AcpiSleep),
            0x63 => Ok(KeyCode::AcpiWake),
            0x65 => Ok(KeyCode::WWWSearch),
            0x66 => Ok(KeyCode::WWWFavorites),
            0x67 => Ok(KeyCode::WWWRefresh),
            0x68 => Ok(KeyCode::WWWStop),
            0x69 => Ok(KeyCode::WWWForward),
            0x6a => Ok(KeyCode::WWWBack),
            0x6b => Ok(KeyCode::MyComputer),
            0x6c => Ok(KeyCode::Email),
            0x6d => Ok(KeyCode::MediaSelect),
            _ => Err(KeyError::UnknownScancode)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyboardState {
    /// The keyboard is not in the middle of any extended key presses
    Start,
    /// An extended key, eg arrow keys, has been pressed, but the press event is not yet over
    Extended
}

/// Holds the state of the currently pressed modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers {
    lctrl: bool,
    rctrl: bool,
    alt: bool,
    alt_gr: bool,
    lshift: bool,
    rshift: bool,
    caps_lock: bool
}

impl KeyModifiers {
    /// Creates a new KeyModifiers instance with all modifiers unset
    fn new() -> Self {
        KeyModifiers {
            lctrl: false,
            rctrl: false,
            alt: false,
            alt_gr: false,
            lshift: false,
            rshift: false,
            caps_lock: false
        }
    }
}

/// A key press or release, together with modifiers
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub keycode: KeyCode,
    pub key_modifiers: KeyModifiers,
    pub direction: KeyDirection
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDirection {
    /// The key is being pressed down
    Down,
    /// The key is being released
    Up
}

/// A code associated with a particular key on the keyboard in scancode set 1
///
/// List gotten from https://wiki.osdev.org/PS/2_Keyboard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Escape,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Zero,
    Dash,
    Equals,
    Backspace,
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    OpenBracket,
    CloseBracket,
    Enter,
    LeftCtrl,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    SemiColon,
    SingleQuote,
    Backtick,
    LeftShift,
    BackSlash,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Dot,
    ForwardSlash,
    RightShift,
    KeypadStar,
    LeftAlt,
    Space,
    CapsLock,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    NumLock,
    ScrollLock,
    KeypadSeven,
    KeypadEight,
    KeypadNine,
    KeypadDash,
    KeypadFour,
    KeypadFive,
    KeypadSix,
    KeypadOne,
    KeypadTwo,
    KeypadThree,
    KeypadPlus,
    KeypadZero,
    KeypadDot,
    PrevTrack,
    NextTrack,
    KeypadEnter,
    RightCtrl,
    Mute,
    Calculator,
    Play,
    Stop,
    VolumeDown,
    VolumeUp,
    WWWHome,
    KeypadForwardSlash,
    AltGr,
    Home,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    PageUp,
    End,
    PageDown,
    Insert,
    Delete,
    LeftGUI,
    RightGUI,
    Apps,
    AcpiPower,
    AcpiSleep,
    AcpiWake,
    WWWSearch,
    WWWFavorites,
    WWWRefresh,
    WWWStop,
    WWWForward,
    WWWBack,
    MyComputer,
    Email,
    MediaSelect
}

impl KeyCode {
    /// Tells whether or not the KeyCode is a modifier
    fn is_modifier(&self) -> bool {
        match *self {
            KeyCode::LeftCtrl | KeyCode::RightCtrl | KeyCode::LeftShift |
            KeyCode::RightShift | KeyCode::LeftAlt | KeyCode::AltGr | KeyCode::CapsLock => true,
            _ => false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    UnknownScancode
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCANCODE_ENTER_PRESS: u8 = 0x1c;
    const SCANCODE_LCTRL_PRESS: u8 = 0x1d;
    const SCANCODE_ALT_GR_RELEASE: u8 = 0xb8;
    const SCANCODE_X_PRESS: u8 = 0x2d;
    const SCANCODE_ARROW_UP_PRESS: u8 = 0x48;
    const SCANCODE_SEMICOLON_RELEASE: u8 = 0xa7;
    const SCANCODE_BAD: u8 = 0xff;

    #[test]
    fn test_enter_press() {
        let mut kbd = Keyboard::new();
        let event = kbd.process_byte(SCANCODE_ENTER_PRESS);
        assert_eq!(event, Ok(Some(KeyEvent {
            keycode: KeyCode::Enter,
            key_modifiers: KeyModifiers::new(),
            direction: KeyDirection::Down
        })));
    }

    #[test]
    fn test_left_ctrl_press() {
        let mut kbd = Keyboard::new();
        let event = kbd.process_byte(SCANCODE_LCTRL_PRESS);
        assert_eq!(event, Ok(None));
    }

    #[test]
    fn test_alt_gr_release() {
        let mut kbd = Keyboard::new();
        kbd.state = KeyboardState::Extended;
        kbd.modifiers.alt_gr = true;
        
        let event = kbd.process_byte(SCANCODE_ALT_GR_RELEASE);
        assert_eq!(event, Ok(None));
        assert!(!kbd.modifiers.alt_gr);
        assert_eq!(kbd.state, KeyboardState::Start);
    }

    #[test]
    fn test_left_shift_x_press() {
        let mut kbd = Keyboard::new();
        kbd.modifiers.lshift = true;

        let event = kbd.process_byte(SCANCODE_X_PRESS);
        let mut expected_modifiers = KeyModifiers::new();
        expected_modifiers.lshift = true;
        assert_eq!(event, Ok(Some(KeyEvent {
            keycode: KeyCode::X,
            key_modifiers: expected_modifiers,
            direction: KeyDirection::Down
        })))
    }

    #[test]
    fn test_arrow_up_down() {
        let mut kbd = Keyboard::new();
        let event1 = kbd.process_byte(EXTENDED_KEY_CODE);
        assert_eq!(event1, Ok(None));
        let event2 = kbd.process_byte(SCANCODE_ARROW_UP_PRESS);
        assert_eq!(event2, Ok(Some(KeyEvent {
            keycode: KeyCode::ArrowUp,
            key_modifiers: KeyModifiers::new(),
            direction: KeyDirection::Down
        })))
    }

    #[test]
    fn test_semicolon_up() {
        let mut kbd = Keyboard::new();
        let event = kbd.process_byte(SCANCODE_SEMICOLON_RELEASE);
        assert_eq!(event, Ok(Some(KeyEvent {
            keycode: KeyCode::SemiColon,
            key_modifiers: KeyModifiers::new(),
            direction: KeyDirection::Up
        })));
    }

    #[test]
    fn test_bad_keycode() {
        let mut kbd = Keyboard::new();
        let event = kbd.process_byte(SCANCODE_BAD);
        assert_eq!(event, Err(KeyError::UnknownScancode));
    }
}
