use std::collections::HashMap;

use crossterm::event::KeyCode;
use shooting_game::input_keyboard::{KeyState, GRACE_PERIOD, HOLD_WINDOW};

fn press(keys: &mut HashMap<KeyCode, KeyState>, key: KeyCode, frame: u64) {
    keys.insert(key, KeyState::Held(frame));
}

fn release(keys: &mut HashMap<KeyCode, KeyState>, key: KeyCode, frame: u64) {
    keys.insert(key, KeyState::Released(frame));
}

fn is_held(keys: &HashMap<KeyCode, KeyState>, key: &KeyCode, frame: u64) -> bool {
    keys.get(key).is_some_and(|s| s.is_held(frame))
}

// ── Basic press / no-press ────────────────────────────────────────────────────

#[test]
fn key_never_pressed_is_not_held() {
    let keys: HashMap<KeyCode, KeyState> = HashMap::new();
    assert!(!is_held(&keys, &KeyCode::Left, 10));
}

#[test]
fn release_without_press_is_not_held() {
    let keys: HashMap<KeyCode, KeyState> = HashMap::new();
    // A key that was never pressed is absent from the map → not held.
    assert!(!is_held(&keys, &KeyCode::Left, 10));
}

#[test]
fn key_pressed_this_frame_is_held() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 10);
    assert!(is_held(&keys, &KeyCode::Left, 10));
}

#[test]
fn key_pressed_within_hold_window_is_held() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 1);
    assert!(is_held(&keys, &KeyCode::Left, 1 + HOLD_WINDOW));
}

#[test]
fn key_pressed_beyond_hold_window_is_not_held() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 1);
    assert!(!is_held(&keys, &KeyCode::Left, 1 + HOLD_WINDOW + 1));
}

// ── Release handling ──────────────────────────────────────────────────────────

#[test]
fn key_released_after_press_is_not_held_past_grace() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 10);
    release(&mut keys, KeyCode::Left, 11);
    // Well outside grace period
    assert!(!is_held(&keys, &KeyCode::Left, 11 + GRACE_PERIOD + 1));
}

#[test]
fn key_released_is_still_held_within_grace_period() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 10);
    release(&mut keys, KeyCode::Left, 11);
    // Exactly at the grace boundary
    assert!(is_held(&keys, &KeyCode::Left, 11 + GRACE_PERIOD));
}

#[test]
fn key_released_is_not_held_one_frame_past_grace() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 10);
    release(&mut keys, KeyCode::Left, 11);
    assert!(!is_held(&keys, &KeyCode::Left, 11 + GRACE_PERIOD + 1));
}

#[test]
fn release_after_press_expires_even_if_hold_window_not_expired() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 10);
    release(&mut keys, KeyCode::Left, 11);
    // Within HOLD_WINDOW, but beyond release grace => not held.
    assert!(!is_held(&keys, &KeyCode::Left, 13));
}

// ── Re-press after release ────────────────────────────────────────────────────

#[test]
fn repress_after_release_is_held() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 10);
    release(&mut keys, KeyCode::Left, 11);
    // Re-press after a while
    press(&mut keys, KeyCode::Left, 20);
    assert!(is_held(&keys, &KeyCode::Left, 20));
}

#[test]
fn press_and_release_same_frame_is_treated_as_held() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 10);
    release(&mut keys, KeyCode::Left, 10);
    // Release at the same frame falls within the grace period.
    assert!(is_held(&keys, &KeyCode::Left, 10));
}

// ── Ghostty false-release scenario ───────────────────────────────────────────
// When Space is pressed while Left is held, Ghostty sends a Release for Left.
// The grace period should keep Left alive for a short tap of Space.

#[test]
fn ghostty_false_release_kept_alive_by_grace() {
    let mut keys = HashMap::new();
    // Left held since frame 1, repeat keeps it fresh
    press(&mut keys, KeyCode::Left, 50);
    // Space pressed at frame 51 — terminal fires a false Release for Left
    release(&mut keys, KeyCode::Left, 51);
    // Exactly at the grace boundary — Left should still appear held
    assert!(is_held(&keys, &KeyCode::Left, 51 + GRACE_PERIOD));
}

#[test]
fn ghostty_false_release_expires_after_grace() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 50);
    release(&mut keys, KeyCode::Left, 51);
    // Past the grace window and no repeat arrived → truly released
    assert!(!is_held(&keys, &KeyCode::Left, 51 + GRACE_PERIOD + 1));
}

// ── Independent keys ─────────────────────────────────────────────────────────

#[test]
fn unrelated_key_does_not_affect_query() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Right, 10);
    assert!(!is_held(&keys, &KeyCode::Left, 10));
}
