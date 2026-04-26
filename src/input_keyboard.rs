use std::collections::HashMap;

use crossterm::event::KeyCode;

/// A key is considered "held" if its last press/repeat event arrived within
/// this many frames — bridges the OS key-repeat initial delay (~7–15 frames).
pub const HOLD_WINDOW: u64 = 20;

/// Frames to keep a key alive after a Release before truly stopping.
///
/// Ghostty (Kitty keyboard protocol) fires a Release for the held movement key
/// the moment a second key (e.g. Space) is pressed, and also stops sending
/// Repeat for that key while the second key is held.  Without a grace period
/// the movement key expires immediately.  6 frames (~200 ms) covers a quick
/// Space tap; if Space is held longer the player briefly stops then resumes.
pub const GRACE_PERIOD: u64 = 6;

/// Returns true if `key` is currently held.
pub fn is_held(
    key_frame: &HashMap<KeyCode, u64>,
    release_frame: &HashMap<KeyCode, u64>,
    key: &KeyCode,
    frame: u64,
) -> bool {
    match key_frame.get(key) {
        None => false,
        Some(&last_press) => {
            if frame.saturating_sub(last_press) > HOLD_WINDOW {
                return false;
            }
            match release_frame.get(key) {
                None => true,
                Some(&last_release) => {
                    // Still held if last press came after the release, OR we are
                    // within the grace period of a (potentially false) release.
                    last_press >= last_release || frame.saturating_sub(last_release) <= GRACE_PERIOD
                }
            }
        }
    }
}
