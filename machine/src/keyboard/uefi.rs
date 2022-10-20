use crate::keyboard::{KeyEvent, KeyDirection, KeyCode, KeyModifiers};

/// A structure that describes key stroke information
///
/// This is the data that is retrieved with UEFI's simple text protocol
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct EFIInputKey {
    pub scancode: EFIScanCode,
    /// The unicode value if the character pressed is printable,
    /// 0 otherwise
    pub unicode_char: u16
}

impl TryFrom<EFIInputKey> for KeyEvent {
    type Error = ();
    fn try_from(key: EFIInputKey) -> Result<KeyEvent, ()> {
        // The EFIInputKey doesn't provide any modifier information
        let mut modifiers = KeyModifiers {
            lctrl: false,
            rctrl: false,
            alt: false,
            alt_gr: false,
            lshift: false,
            rshift: false,
            caps_lock: false
        };
        // UEFI's simple text protocol doesn't give information
        // on key ups
        let direction = KeyDirection::Down;
        // All EFIInputKeys that can be represented as unicode characters
        // will have a non-zero unicode_char field and a EFIScanCode::Null
        // as their scancode. Keys that can't be represented with the unicode
        // will be represented with the scancode and have a unicode_char field of 0
        let keycode: KeyCode;
        // We can tell if shift is down if the character code represents a
        // character that requires shift to be inputted. For example, shift + 1
        // is "!"
        let mut shift_down = false;
        if key.scancode == EFIScanCode::Null {
            if key.unicode_char >= 32 && key.unicode_char <= 47 {
                (shift_down, keycode) = map_ascii_punctuation_1(key.unicode_char);
            } else if key.unicode_char >= 48 && key.unicode_char <= 57 {
                keycode = map_ascii_number(key.unicode_char);
            } else if key.unicode_char >= 58 && key.unicode_char <= 64 {
                (shift_down, keycode) = map_ascii_punctuation_2(key.unicode_char);
            } else if key.unicode_char >= 65 && key.unicode_char <= 90 {
                // An approximation. UEFI's simple text protocol doesn't tell
                // when shift is pressed. 
                modifiers.caps_lock = true;
                keycode = map_latin_uppercase_alphabet(key.unicode_char);
            } else if key.unicode_char >= 91 && key.unicode_char <= 96 {
                (shift_down, keycode) = map_ascii_punctuation_3(key.unicode_char);
            } else if key.unicode_char >= 97 && key.unicode_char <= 122 {
                keycode = map_latin_lowercase_alphabet(key.unicode_char);
            } else if key.unicode_char <= 31 {
                keycode = map_control_char(key.unicode_char)?;
            } else {
                return Err(());
            }
        } else {
            keycode = map_efi_scancode(key.scancode)?;
        }
        modifiers.lshift = shift_down;
        Ok(KeyEvent {
            keycode,
            direction,
            key_modifiers: modifiers
        })
    }
}

/// This is the data that is retrieved with UEFI's extended
/// simple text protocol
#[derive(Debug)]
#[repr(C)]
pub struct EFIKeyData {
    /// The EFI scancode and unicode values from the input device
    key: EFIInputKey,
    /// The current state of input modifiers and toggle values
    key_state: EFIInputKeyState
}

#[derive(Debug)]
#[repr(C)]
struct EFIInputKeyState {
    /// Reflects the currently pressed modifiers for the input device
    ///
    /// The value is valid only if the high order bit has been set
    key_modifiers: EFIKeyModifiers,
    /// Reflects the current internal state of various toggled attributes
    ///
    /// The returned value is valid only if the high order bit has been set
    key_toggle_state: EFIKeyToggle
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
enum EFIKeyModifiers {
    None                = 0x00,
    RightShiftPressed   = 0x01,
    LeftShiftPressed    = 0x02,
    RightCtrlPressed    = 0x04,
    LeftCtrlPressed     = 0x08,
    RightAltPresssed    = 0x10,
    LeftAltPressed      = 0x20,
    RightLogoPressed    = 0x40,
    LeftLogoPressed     = 0x80,
    MenuKeyPressed      = 0x100,
    SysReqPressed       = 0x200
}

impl EFIKeyModifiers {
    const MODIFIER_STATE_VALID: u32 = 0x80000000;
    /// Checks if the value is valid
    ///
    /// According to the UEFI spec, a value is valid if the
    /// high order bit is set
    fn is_valid(&self) -> bool {
        *self as u32 & Self::MODIFIER_STATE_VALID == Self::MODIFIER_STATE_VALID
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum EFIKeyToggle {
    None                    = 0x00,
    ToggleStateValid        = 0x80,
    KeyStateExposed         = 0x40,
    ScrollLockActive        = 0x01,
    NumLockActive           = 0x02,
    CapsLockActive          = 0x04
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u16)]
pub enum EFIScanCode {
    Null        = 0x00,
    CursorUp    = 0x01,
    CursorDown  = 0x02,
    CursorRight = 0x03,
    CursorLeft  = 0x04,
    Home        = 0x05,
    End         = 0x06,
    Insert      = 0x07,
    Delete      = 0x08,
    PageUp      = 0x09,
    PageDown    = 0x0a,
    F1          = 0x0b,
    F2          = 0x0c,
    F3          = 0x0d,
    F4          = 0x0e,
    F5          = 0x0f,
    F6          = 0x10,
    F7          = 0x11,
    F8          = 0x12,
    F9          = 0x13,
    F10         = 0x14,
    Escape      = 0x17,
    F11         = 0x15,
    F12         = 0x16,
    F13         = 0x68,
    F14         = 0x69,
    F15         = 0x6a,
    F16         = 0x6b,
    F17         = 0x6c,
    F18         = 0x6d,
    F19         = 0x6e,
    F20         = 0x6f,
    F21         = 0x70,
    F22         = 0x71,
    F23         = 0x72,
    F24         = 0x73,
    Mute        = 0x7f,
    VolumeUp    = 0x80,
    VolumeDown  = 0x81,
    BrightnessUp = 0x100,
    BrightnessDown = 0x101,
    Suspend     = 0x102,
    Hibernate   = 0x103,
    ToggleDisplay   = 0x104,
    Recovery    = 0x105,
    Eject       = 0x106
    // OEM Reserved: 0x8000 - 0xffff
}

/// Maps ascii punctuations in the range 32..=47 to KeyCodes
/// The boolean in the tuple tells if shift is down. For example,
/// "!" is number 33 and [shift + 1] keys are required to get this code
fn map_ascii_punctuation_1(code: u16) -> (bool, KeyCode) {
    let shift_is_down = match code {
        33..=38 => true,
        40..=43 => true,
        _ => false
    };
    let keycode = match code {
        32 => KeyCode::Space,
        33 => KeyCode::One,
        34 => KeyCode::SingleQuote,
        35 => KeyCode::Three,
        36 => KeyCode::Four,
        37 => KeyCode::Five,
        38 => KeyCode::Seven,
        39 => KeyCode::SingleQuote,
        40 => KeyCode::Nine,
        41 => KeyCode::Zero,
        42 => KeyCode::Eight,
        43 => KeyCode::Equals,
        44 => KeyCode::Comma,
        45 => KeyCode::Dash,
        46 => KeyCode::Dot,
        47 => KeyCode::ForwardSlash,
        _ => unreachable!()
    };
    (shift_is_down, keycode)
}

fn map_ascii_number(code: u16) -> KeyCode {
    match code {
        48 => KeyCode::Zero,
        49 => KeyCode::One,
        50 => KeyCode::Two,
        51 => KeyCode::Three,
        52 => KeyCode::Four,
        53 => KeyCode::Five,
        54 => KeyCode::Six,
        55 => KeyCode::Seven,
        56 => KeyCode::Eight,
        57 => KeyCode::Nine,
        _ => unreachable!()
    }
}

fn map_ascii_punctuation_2(code: u16) -> (bool, KeyCode) {
    let shift_is_down = match code {
        58 => true,
        60 => true,
        62..=64 => true,
        _ => false
    };
    let keycode = match code {
        58 => KeyCode::SemiColon,
        59 => KeyCode::SemiColon,
        60 => KeyCode::Comma,
        61 => KeyCode::Equals,
        62 => KeyCode::Dot,
        63 => KeyCode::ForwardSlash,
        64 => KeyCode::Two,
        _ => unreachable!()
    };
    (shift_is_down, keycode)
}

fn map_latin_uppercase_alphabet(code: u16) -> KeyCode {
    match code {
        65 => KeyCode::A,
        66 => KeyCode::B,
        67 => KeyCode::C,
        68 => KeyCode::D,
        69 => KeyCode::E,
        70 => KeyCode::F,
        71 => KeyCode::G,
        72 => KeyCode::H,
        73 => KeyCode::I,
        74 => KeyCode::J,
        75 => KeyCode::K,
        76 => KeyCode::L,
        77 => KeyCode::M,
        78 => KeyCode::N,
        79 => KeyCode::O,
        80 => KeyCode::P,
        81 => KeyCode::Q,
        82 => KeyCode::R,
        83 => KeyCode::S,
        84 => KeyCode::T,
        85 => KeyCode::U,
        86 => KeyCode::V,
        87 => KeyCode::W,
        88 => KeyCode::X,
        89 => KeyCode::Y,
        90 => KeyCode::Z,
        _ => unreachable!()
    }
}

fn map_ascii_punctuation_3(code: u16) -> (bool, KeyCode) {
    let shift_is_down = code == 94 || code == 95;
    let keycode = match code {
        91 => KeyCode::OpenBracket,
        92 => KeyCode::BackSlash,
        93 => KeyCode::CloseBracket,
        94 => KeyCode::Six,
        95 => KeyCode::Dash,
        96 => KeyCode::Backtick,
        _ => unreachable!()
    };
    (shift_is_down, keycode)
}

fn map_latin_lowercase_alphabet(code: u16) -> KeyCode {
    map_latin_uppercase_alphabet(code - (97 - 65))
}

fn map_efi_scancode(code: EFIScanCode) -> Result<KeyCode, ()> {
    match code {
        EFIScanCode::CursorUp => Ok(KeyCode::ArrowUp),
        EFIScanCode::CursorDown => Ok(KeyCode::ArrowDown),
        EFIScanCode::CursorLeft => Ok(KeyCode::ArrowLeft),
        EFIScanCode::CursorRight => Ok(KeyCode::ArrowRight),
        EFIScanCode::Home => Ok(KeyCode::Home),
        EFIScanCode::End => Ok(KeyCode::End),
        EFIScanCode::Insert => Ok(KeyCode::Insert),
        EFIScanCode::Delete => Ok(KeyCode::Delete),
        EFIScanCode::PageUp => Ok(KeyCode::PageUp),
        EFIScanCode::PageDown => Ok(KeyCode::PageDown),
        EFIScanCode::Escape => Ok(KeyCode::Escape),
        _ => Err(())
    }
}

fn map_control_char(code: u16) -> Result<KeyCode, ()> {
    match code {
        8 => Ok(KeyCode::Backspace),
        9 => Ok(KeyCode::Tab),
        10 => Ok(KeyCode::Enter),
        13 => Ok(KeyCode::Enter),
        _ => Err(())
    }
}