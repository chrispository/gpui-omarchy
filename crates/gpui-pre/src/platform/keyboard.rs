use collections::HashMap;

use crate::{Capslock, KeybindingKeystroke, Keystroke, Modifiers};

/// A trait for platform-specific keyboard layouts
pub trait PlatformKeyboardLayout {
    /// Get the keyboard layout ID, which should be unique to the layout
    fn id(&self) -> &str;
    /// Get the keyboard layout display name
    fn name(&self) -> &str;
}

/// A trait for platform-specific keyboard mappings
pub trait PlatformKeyboardMapper {
    /// Raw non-keypad events (including the complete main block) and effective held modifiers, before symbol Shift normalization; cache by layout generation, requested modifiers and Caps Lock.
    /// None means unsupported, including Linux's unobservable function modifier; AltGr is not represented by Modifiers. These are pre-composition strokes, so consumers must retain native IME/prefer_character_input guards when deciding dispatch reachability.
    fn shortcut_samples(
        &self,
        _modifiers: Modifiers,
        _capslock: Capslock,
    ) -> Option<Vec<(Keystroke, Modifiers)>> {
        None
    }

    /// Map a key equivalent to its platform-specific representation
    fn map_key_equivalent(
        &self,
        keystroke: Keystroke,
        use_key_equivalents: bool,
    ) -> KeybindingKeystroke;
    /// Get the key equivalents for the current keyboard layout,
    /// only used on macOS
    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>>;
}

/// A dummy implementation of the platform keyboard mapper
pub struct DummyKeyboardMapper;

impl PlatformKeyboardMapper for DummyKeyboardMapper {
    fn map_key_equivalent(
        &self,
        keystroke: Keystroke,
        _use_key_equivalents: bool,
    ) -> KeybindingKeystroke {
        KeybindingKeystroke::from_keystroke(keystroke)
    }

    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>> {
        None
    }
}
