/// A key is considered "held" if its last press/repeat event arrived within
/// this many frames.  3 frames (~100 ms) is enough to stay live between
/// consecutive Repeat events while expiring quickly after physical release.
pub const HOLD_WINDOW: u64 = 5;

/// Frames to keep a key alive after a Release before truly stopping.
///
/// Ghostty (Kitty keyboard protocol) fires a Release for the held movement key
/// the moment a second key (e.g. Space) is pressed, and also stops sending
/// Repeat for that key while the second key is held.  Without a grace period
/// the movement key expires immediately.  1 frame (~33 ms) covers a quick
/// Space tap; if Space is held longer the player briefly stops then resumes.
pub const GRACE_PERIOD: u64 = 1;

/// Per-key input state tracked in the game loop's key map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
    /// Key was last pressed/repeated at this frame.
    Held(u64),
    /// Key was released at this frame; grace period may still keep it live.
    Released(u64),
}

impl KeyState {
    /// Returns true if the key should be treated as currently held.
    pub fn is_held(&self, frame: u64) -> bool {
        match self {
            KeyState::Held(last) => frame.saturating_sub(*last) <= HOLD_WINDOW,
            KeyState::Released(at) => frame.saturating_sub(*at) <= GRACE_PERIOD,
        }
    }

    /// Returns the frame number if the key is in the `Held` state, else `None`.
    /// Used to detect rapid re-press (classic-terminal OS key-repeat simulation).
    pub fn as_held_frame(&self) -> Option<u64> {
        if let KeyState::Held(f) = self {
            Some(*f)
        } else {
            None
        }
    }
}
