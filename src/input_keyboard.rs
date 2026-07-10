use std::collections::HashMap;

use crossterm::event::KeyCode;

/// A key is considered "held" if its last press/repeat event arrived within
/// this many frames.  5 frames (~165 ms) is enough to stay live between
/// consecutive repeat events while expiring quickly after physical release.
pub const HOLD_WINDOW: u64 = 5;

/// Frames to keep a key alive after a Release before truly stopping.
///
/// Ghostty (Kitty keyboard protocol) fires a Release for the held movement key
/// the moment a second key (e.g. Space) is pressed, and also stops sending
/// Repeat for that key while the second key is held.  Without a grace period
/// the movement key expires immediately.  1 frame (~33 ms) covers a quick
/// Space tap; if Space is held longer the player briefly stops then resumes.
pub const GRACE_PERIOD: u64 = 1;

/// Max frames between two Presses of the same direction key for the second
/// press to count as key-repeat.  Classic terminals (and tmux with
/// extended-keys off) report OS key-repeat as repeated Press events; a rapid
/// re-press engages continuous movement.
pub const RAPID_WINDOW: u64 = 4;

/// Enhanced-mode safety net: if NO movement-related event (direction key,
/// F, or W) arrives for this many frames, drop the engaged direction even
/// though no Release was seen.  This bounds the rare case where a true
/// Release is swallowed by the false-release heuristic (direction released
/// in the exact frame another key was pressed).  ~1 second at 30 FPS.
pub const ENHANCED_SILENCE_LIMIT: u64 = 30;

/// Classic-mode engagement memory: how long an engaged direction survives
/// movement-event silence before it is dropped for good.  Movement itself
/// pauses after `HOLD_WINDOW` frames, but the engagement is remembered a
/// while longer so it can RESUME when events come back.  This bridges the
/// OS initial key-repeat delay (~250–500 ms): pressing W while gliding
/// silences the direction key immediately, and W's own repeats only start
/// after that delay — the combo must survive the gap.  ~500 ms at 30 FPS.
pub const CLASSIC_ENGAGE_MEMORY: u64 = 15;

/// Per-key input state tracked in the input tracker's key map.
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
    /// Used to detect rapid re-press (classic-terminal OS key-repeat).
    pub fn as_held_frame(&self) -> Option<u64> {
        if let KeyState::Held(f) = self {
            Some(*f)
        } else {
            None
        }
    }
}

/// Horizontal movement direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
}

/// Maps a key code to the movement direction it controls, if any.
fn dir_of(code: KeyCode) -> Option<Dir> {
    match code {
        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => Some(Dir::Left),
        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => Some(Dir::Right),
        _ => None,
    }
}

/// Keys whose activity keeps an engaged direction alive: the direction keys
/// themselves plus the movement modifiers F (fast) and W (warp).  When F/W is
/// held, both Ghostty and classic OS key-repeat stop emitting events for the
/// direction key, so F/W events are the only proof the combo is still active.
/// Space is deliberately excluded: holding Space to autofire after releasing
/// a direction key must NOT keep the ship sliding.
fn refreshes_movement(code: KeyCode) -> bool {
    dir_of(code).is_some()
        || matches!(
            code,
            KeyCode::Char('f') | KeyCode::Char('F') | KeyCode::Char('w') | KeyCode::Char('W')
        )
}

/// Pure input state machine for the game loop.
///
/// `enhanced` selects the trust model:
/// * **Enhanced** (Ghostty raw — verified via the kitty-protocol query):
///   Release events are reliably delivered, so an engaged direction persists
///   until a true Release clears it.  A long silence safety net
///   (`ENHANCED_SILENCE_LIMIT`) bounds the rare swallowed-release case.
/// * **Classic** (tmux with extended-keys off, plain terminals): Release is
///   NEVER delivered; the only stop signal is silence.  An engaged direction
///   expires `HOLD_WINDOW` frames (~165 ms) after the last movement-related
///   event.  OS key-repeat keeps Presses flowing while a key is held; F/W
///   repeats keep a combo alive while they are held.
///
/// Usage per frame: `begin_drain(frame)`, then `record_press` /
/// `record_repeat` / `record_release` for each event, then
/// `end_drain(frame)`.  Query with `active_dir(frame)` / `is_held(code, frame)`.
#[derive(Debug)]
pub struct InputTracker {
    enhanced: bool,
    keys: HashMap<KeyCode, KeyState>,
    held_dir: Option<Dir>,
    /// Classic mode only: the engaged direction key is believed physically
    /// held even though the terminal has stopped sending events for it.
    /// Entered when F/W is pressed mid-glide — OS key-repeat switches to the
    /// newest key permanently (it never resumes for the direction key, not
    /// even after F/W is released), so the direction key falls silent while
    /// still held, and its release is equally silent.  With no signal either
    /// way, silence stops meaning "released": the freshness gates are
    /// bypassed and the engagement stays on trust until the next
    /// direction-key event (a tap stops the glide).
    phantom: bool,
    /// Frame of the last movement-related Press/Repeat (see `refreshes_movement`).
    last_movement_activity: u64,
    deferred_releases: Vec<KeyCode>,
    has_non_dir_press: bool,
}

impl InputTracker {
    pub fn new(enhanced: bool) -> Self {
        Self {
            enhanced,
            keys: HashMap::new(),
            held_dir: None,
            phantom: false,
            last_movement_activity: 0,
            deferred_releases: Vec::new(),
            has_non_dir_press: false,
        }
    }

    /// Silence tolerance for APPLYING movement (the active gate).  In classic
    /// mode this is what stops the player after a key release: short, so the
    /// post-release slide stays imperceptible.
    fn active_limit(&self) -> u64 {
        if self.enhanced {
            ENHANCED_SILENCE_LIMIT
        } else {
            HOLD_WINDOW
        }
    }

    /// Silence tolerance for REMEMBERING the engagement.  Longer than the
    /// active gate in classic mode so a paused combo can resume when F/W
    /// repeats arrive after the OS initial key-repeat delay.
    fn memory_limit(&self) -> u64 {
        if self.enhanced {
            ENHANCED_SILENCE_LIMIT
        } else {
            CLASSIC_ENGAGE_MEMORY
        }
    }

    fn movement_gap(&self, frame: u64) -> u64 {
        frame.saturating_sub(self.last_movement_activity)
    }

    /// Reset per-drain flags and expire a stale engagement.  Call once per
    /// frame before processing events.  The expiry here is sticky: once the
    /// memory limit passes, the engagement is gone and a later unrelated
    /// key press cannot resurrect it.
    pub fn begin_drain(&mut self, frame: u64) {
        self.deferred_releases.clear();
        self.has_non_dir_press = false;
        // A phantom engagement never expires on silence: the terminal sends
        // nothing for that key while it is held, so only a Release ends it.
        if !self.phantom
            && self.held_dir.is_some()
            && self.movement_gap(frame) > self.memory_limit()
        {
            self.held_dir = None;
        }
    }

    /// Record a Press event.  Returns true if it was a rapid re-press of a
    /// direction key (classic-terminal key-repeat): continuous movement takes
    /// over, so the caller should NOT apply a single-step move.
    pub fn record_press(&mut self, code: KeyCode, frame: u64) -> bool {
        if refreshes_movement(code) {
            self.last_movement_activity = frame;
        }
        let rapid = match dir_of(code) {
            Some(dir) => {
                let rapid = self
                    .keys
                    .get(&code)
                    .and_then(|s| s.as_held_frame())
                    .is_some_and(|last| frame.saturating_sub(last) <= RAPID_WINDOW);
                self.held_dir = if rapid { Some(dir) } else { None };
                // Direction events are flowing again → back to normal tracking.
                self.phantom = false;
                rapid
            }
            None => {
                self.has_non_dir_press = true;
                // Classic terminals: an F/W press steals the OS key-repeat
                // from the direction key for good (no resume on release).
                // From here on silence is expected, not a stop signal — the
                // engagement goes phantom.  Enhanced terminals handle this
                // via the false-release path in end_drain instead.
                if !self.enhanced && refreshes_movement(code) && self.held_dir.is_some() {
                    self.phantom = true;
                }
                false
            }
        };
        self.keys.insert(code, KeyState::Held(frame));
        rapid
    }

    /// Record a Repeat event: refresh the key and engage its direction.
    pub fn record_repeat(&mut self, code: KeyCode, frame: u64) {
        if refreshes_movement(code) {
            self.last_movement_activity = frame;
        }
        if let Some(dir) = dir_of(code) {
            self.held_dir = Some(dir);
            // Direction events are flowing again → back to normal tracking.
            self.phantom = false;
        }
        self.keys.insert(code, KeyState::Held(frame));
    }

    /// Record a Release event.  Deferred until `end_drain` so a straggler
    /// Repeat in the same flush cannot re-enable a just-released key.
    pub fn record_release(&mut self, code: KeyCode) {
        self.deferred_releases.push(code);
    }

    /// Apply the deferred releases for this drain.
    ///
    /// Ghostty fires a *false* Release for a held direction key the moment any
    /// other key is pressed.  When a non-direction Press arrived in this same
    /// drain, direction releases are treated as false: `held_dir` survives and
    /// the key's `Held` timestamp is kept.  If the key really was released,
    /// the silence safety net eventually drops the engagement.
    pub fn end_drain(&mut self, frame: u64) {
        let releases = std::mem::take(&mut self.deferred_releases);
        for code in releases {
            match dir_of(code) {
                // False release: keep held_dir and the Held timestamp.
                Some(_) if self.has_non_dir_press => {}
                Some(dir) => {
                    if self.held_dir == Some(dir) {
                        self.held_dir = None;
                        self.phantom = false;
                    }
                    self.keys.insert(code, KeyState::Released(frame));
                }
                None => {
                    self.keys.insert(code, KeyState::Released(frame));
                }
            }
        }
    }

    /// Returns true if `code` is currently held (fresh press/repeat, or
    /// within the release grace period).
    pub fn is_held(&self, code: &KeyCode, frame: u64) -> bool {
        self.keys.get(code).is_some_and(|s| s.is_held(frame))
    }

    /// The direction the player is actively moving, if any.
    ///
    /// An engaged direction stays active while movement-related events keep
    /// arriving (direction keys, or F/W whose own repeats prove a combo is
    /// still held).  In classic mode silence beyond `HOLD_WINDOW` pauses the
    /// player — the only stop signal that exists without Release events —
    /// while the engagement itself is remembered for `CLASSIC_ENGAGE_MEMORY`
    /// frames so a combo paused by the OS key-repeat delay can resume.  In
    /// enhanced mode a true Release stops the player instantly and the long
    /// silence limit is only a safety net.  A phantom engagement bypasses
    /// the freshness gate entirely: the key is silent BECAUSE it is held.
    pub fn active_dir(&self, frame: u64) -> Option<Dir> {
        self.held_dir
            .filter(|_| self.phantom || self.movement_gap(frame) <= self.active_limit())
    }

    /// Raw engaged direction without the freshness gate (debug overlay).
    pub fn raw_dir(&self) -> Option<Dir> {
        self.held_dir
    }

    /// True if a non-direction key was pressed in the current drain (debug).
    pub fn non_dir_press_seen(&self) -> bool {
        self.has_non_dir_press
    }

    /// True if the engaged direction is in the phantom state (debug).
    pub fn is_phantom(&self) -> bool {
        self.phantom
    }
}
