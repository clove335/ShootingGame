use std::collections::HashMap;

use crossterm::event::KeyCode;
use shooting_game::input_keyboard::{is_held, GRACE_PERIOD, HOLD_WINDOW};

fn press(key_frame: &mut HashMap<KeyCode, u64>, key: KeyCode, frame: u64) {
    key_frame.insert(key, frame);
}

fn release(release_frame: &mut HashMap<KeyCode, u64>, key: KeyCode, frame: u64) {
    release_frame.insert(key, frame);
}

// ── Basic press / no-press ────────────────────────────────────────────────────

#[test]
fn key_never_pressed_is_not_held() {
    let kf: HashMap<KeyCode, u64> = HashMap::new();
    let rf: HashMap<KeyCode, u64> = HashMap::new();
    assert!(!is_held(&kf, &rf, &KeyCode::Left, 10));
}

#[test]
fn key_pressed_this_frame_is_held() {
    let mut kf = HashMap::new();
    let rf = HashMap::new();
    press(&mut kf, KeyCode::Left, 10);
    assert!(is_held(&kf, &rf, &KeyCode::Left, 10));
}

#[test]
fn key_pressed_within_hold_window_is_held() {
    let mut kf = HashMap::new();
    let rf = HashMap::new();
    press(&mut kf, KeyCode::Left, 1);
    assert!(is_held(&kf, &rf, &KeyCode::Left, 1 + HOLD_WINDOW));
}

#[test]
fn key_pressed_beyond_hold_window_is_not_held() {
    let mut kf = HashMap::new();
    let rf = HashMap::new();
    press(&mut kf, KeyCode::Left, 1);
    assert!(!is_held(&kf, &rf, &KeyCode::Left, 1 + HOLD_WINDOW + 1));
}

// ── Release handling ──────────────────────────────────────────────────────────

#[test]
fn key_released_after_press_is_not_held_past_grace() {
    let mut kf = HashMap::new();
    let mut rf = HashMap::new();
    press(&mut kf, KeyCode::Left, 10);
    release(&mut rf, KeyCode::Left, 11);
    // Well outside grace period
    assert!(!is_held(&kf, &rf, &KeyCode::Left, 11 + GRACE_PERIOD + 1));
}

#[test]
fn key_released_is_still_held_within_grace_period() {
    let mut kf = HashMap::new();
    let mut rf = HashMap::new();
    press(&mut kf, KeyCode::Left, 10);
    release(&mut rf, KeyCode::Left, 11);
    // Exactly at the grace boundary
    assert!(is_held(&kf, &rf, &KeyCode::Left, 11 + GRACE_PERIOD));
}

#[test]
fn key_released_is_not_held_one_frame_past_grace() {
    let mut kf = HashMap::new();
    let mut rf = HashMap::new();
    press(&mut kf, KeyCode::Left, 10);
    release(&mut rf, KeyCode::Left, 11);
    assert!(!is_held(&kf, &rf, &KeyCode::Left, 11 + GRACE_PERIOD + 1));
}

// ── Re-press after release ────────────────────────────────────────────────────

#[test]
fn repress_after_release_is_held() {
    let mut kf = HashMap::new();
    let mut rf = HashMap::new();
    press(&mut kf, KeyCode::Left, 10);
    release(&mut rf, KeyCode::Left, 11);
    // Re-press after a while
    press(&mut kf, KeyCode::Left, 20);
    assert!(is_held(&kf, &rf, &KeyCode::Left, 20));
}

// ── Ghostty false-release scenario ───────────────────────────────────────────
// When Space is pressed while Left is held, Ghostty sends a Release for Left.
// The grace period should keep Left alive for a short tap of Space.

#[test]
fn ghostty_false_release_kept_alive_by_grace() {
    let mut kf = HashMap::new();
    let mut rf = HashMap::new();
    // Left held since frame 1, repeat keeps it fresh
    press(&mut kf, KeyCode::Left, 50);
    // Space pressed at frame 51 — terminal fires a false Release for Left
    release(&mut rf, KeyCode::Left, 51);
    // Two frames later Left should still appear held (grace covers it)
    assert!(is_held(&kf, &rf, &KeyCode::Left, 53));
}

#[test]
fn ghostty_false_release_expires_after_grace() {
    let mut kf = HashMap::new();
    let mut rf = HashMap::new();
    press(&mut kf, KeyCode::Left, 50);
    release(&mut rf, KeyCode::Left, 51);
    // Past the grace window and no repeat arrived → truly released
    assert!(!is_held(&kf, &rf, &KeyCode::Left, 51 + GRACE_PERIOD + 1));
}

// ── Independent keys ─────────────────────────────────────────────────────────

#[test]
fn unrelated_key_does_not_affect_query() {
    let mut kf = HashMap::new();
    let rf = HashMap::new();
    press(&mut kf, KeyCode::Right, 10);
    assert!(!is_held(&kf, &rf, &KeyCode::Left, 10));
}
