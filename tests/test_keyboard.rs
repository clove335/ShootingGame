use std::collections::HashMap;

use crossterm::event::KeyCode;
use shooting_game::input_keyboard::{
    Dir, InputTracker, KeyState, ENHANCED_SILENCE_LIMIT, GRACE_PERIOD, HOLD_WINDOW, RAPID_WINDOW,
};

// ════════════════════════════════════════════════════════════════════════════
// KeyState — low-level freshness primitives
// ════════════════════════════════════════════════════════════════════════════

fn press(keys: &mut HashMap<KeyCode, KeyState>, key: KeyCode, frame: u64) {
    keys.insert(key, KeyState::Held(frame));
}

fn release(keys: &mut HashMap<KeyCode, KeyState>, key: KeyCode, frame: u64) {
    keys.insert(key, KeyState::Released(frame));
}

fn is_held(keys: &HashMap<KeyCode, KeyState>, key: &KeyCode, frame: u64) -> bool {
    keys.get(key).is_some_and(|s| s.is_held(frame))
}

#[test]
fn key_never_pressed_is_not_held() {
    let keys: HashMap<KeyCode, KeyState> = HashMap::new();
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

#[test]
fn key_released_is_still_held_within_grace_period() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 10);
    release(&mut keys, KeyCode::Left, 11);
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
fn repress_after_release_is_held() {
    let mut keys = HashMap::new();
    press(&mut keys, KeyCode::Left, 10);
    release(&mut keys, KeyCode::Left, 11);
    press(&mut keys, KeyCode::Left, 20);
    assert!(is_held(&keys, &KeyCode::Left, 20));
}

// ════════════════════════════════════════════════════════════════════════════
// InputTracker — movement state machine
//
// Event vocabulary per frame (drain cycle):
//   begin_drain(frame) → record_press/record_repeat/record_release → end_drain(frame)
//
// Two terminal classes:
// * Enhanced — InputTracker::new(true) — raw Ghostty: Press + Repeat + Release
//   all delivered (verified by the kitty-protocol query at startup).
// * Classic — InputTracker::new(false) — tmux with extended-keys off, plain
//   terminals: ONLY Press events, never Release; OS key-repeat arrives as
//   rapid re-Presses and only for the MOST RECENTLY pressed key.
// ════════════════════════════════════════════════════════════════════════════

fn classic() -> InputTracker {
    InputTracker::new(false)
}

fn enhanced() -> InputTracker {
    InputTracker::new(true)
}

/// Run one drain cycle with the given events.
/// Events: ('P', code) = press, ('T', code) = repeat, ('X', code) = release.
fn drain(t: &mut InputTracker, frame: u64, events: &[(char, KeyCode)]) {
    t.begin_drain(frame);
    for &(kind, code) in events {
        match kind {
            'P' => {
                t.record_press(code, frame);
            }
            'T' => t.record_repeat(code, frame),
            'X' => t.record_release(code),
            _ => unreachable!(),
        }
    }
    t.end_drain(frame);
}

/// Run empty drain cycles (frames with no input events).
fn idle(t: &mut InputTracker, frames: std::ops::RangeInclusive<u64>) {
    for f in frames {
        drain(t, f, &[]);
    }
}

// ── Engagement ────────────────────────────────────────────────────────────────

#[test]
fn single_press_does_not_engage_continuous_movement() {
    let mut t = classic();
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    assert_eq!(t.active_dir(10), None);
}

#[test]
fn single_press_reports_not_rapid() {
    let mut t = classic();
    t.begin_drain(10);
    let rapid = t.record_press(KeyCode::Left, 10);
    t.end_drain(10);
    assert!(!rapid);
}

#[test]
fn rapid_double_press_engages_movement() {
    let mut t = classic();
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    drain(&mut t, 10 + RAPID_WINDOW, &[('P', KeyCode::Left)]);
    assert_eq!(t.active_dir(10 + RAPID_WINDOW), Some(Dir::Left));
}

#[test]
fn slow_double_press_does_not_engage() {
    let mut t = classic();
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    drain(&mut t, 10 + RAPID_WINDOW + 1, &[('P', KeyCode::Left)]);
    assert_eq!(t.active_dir(10 + RAPID_WINDOW + 1), None);
}

#[test]
fn repeat_event_engages_movement() {
    let mut t = enhanced();
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    drain(&mut t, 12, &[('T', KeyCode::Left)]);
    assert_eq!(t.active_dir(12), Some(Dir::Left));
}

// ── Classic terminal (tmux): release produces no event ───────────────────────
// The original bug: double-press ←, start moving, release the key.  tmux with
// extended-keys off never delivers a Release event, so the only stop signal
// is silence.  Movement must expire within HOLD_WINDOW frames.

#[test]
fn classic_release_silence_stops_movement() {
    let mut t = classic();
    // Double-press engages continuous movement.
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    drain(&mut t, 12, &[('P', KeyCode::Left)]);
    assert_eq!(t.active_dir(12), Some(Dir::Left));

    // Key physically released → NO event arrives, just silence.
    idle(&mut t, 13..=12 + HOLD_WINDOW);
    assert_eq!(t.active_dir(12 + HOLD_WINDOW), Some(Dir::Left)); // still gliding
    idle(&mut t, 12 + HOLD_WINDOW + 1..=12 + HOLD_WINDOW + 1);
    assert_eq!(t.active_dir(12 + HOLD_WINDOW + 1), None); // stopped ≤ ~165 ms
    idle(&mut t, 12 + HOLD_WINDOW + 2..=100);
    assert_eq!(t.active_dir(100), None); // no zombie movement later
}

#[test]
fn classic_os_repeat_presses_sustain_movement() {
    let mut t = classic();
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    // OS key-repeat: a press every frame keeps the glide alive.
    for f in 11..30 {
        drain(&mut t, f, &[('P', KeyCode::Left)]);
        assert_eq!(t.active_dir(f), Some(Dir::Left), "frame {}", f);
    }
    // Repeats stop (key released) → movement expires within the window.
    idle(&mut t, 30..=29 + HOLD_WINDOW + 1);
    assert_eq!(t.active_dir(29 + HOLD_WINDOW + 1), None);
}

#[test]
fn classic_expiry_is_sticky_no_resurrection_by_other_keys() {
    let mut t = classic();
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    drain(&mut t, 12, &[('P', KeyCode::Left)]);
    // Silence well past the window → engagement is gone for good.
    idle(&mut t, 13..=40);
    assert_eq!(t.active_dir(40), None);
    // A later F press refreshes movement activity but must NOT resurrect
    // the stale direction.
    drain(&mut t, 41, &[('P', KeyCode::Char('f'))]);
    assert_eq!(t.active_dir(41), None);
}

// ── Classic terminal: F/W combos (the regression case) ───────────────────────
// While F or W is held, OS key-repeat switches to the NEWEST key: the
// direction key goes silent even though it is still physically held.  F/W
// events themselves must keep the engaged direction alive.

#[test]
fn classic_w_press_keeps_direction_active() {
    let mut t = classic();
    // Glide left via OS repeat presses.
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    for f in 11..=14 {
        drain(&mut t, f, &[('P', KeyCode::Left)]);
    }
    assert_eq!(t.active_dir(14), Some(Dir::Left));

    // W pressed mid-glide: direction key goes silent from here on (OS repeat
    // switches to the newest key) → the engagement goes phantom.
    drain(&mut t, 15, &[('P', KeyCode::Char('w'))]);
    assert_eq!(t.active_dir(15), Some(Dir::Left)); // warp fires with direction

    // OS initial repeat delay (~12 frames): no events at all.  The phantom
    // engagement glides straight through the gap — no momentary stop.
    idle(&mut t, 16..=26);
    assert_eq!(t.active_dir(26), Some(Dir::Left));

    // W's own repeats arrive; the combo keeps moving.
    for f in 27..=40 {
        drain(&mut t, f, &[('P', KeyCode::Char('w'))]);
        assert_eq!(t.active_dir(f), Some(Dir::Left), "frame {}", f);
    }

    // W released, Left still physically held: total silence — the glide
    // must CONTINUE (this was the bug: it stopped within ~0.5 s).
    idle(&mut t, 41..=200);
    assert_eq!(t.active_dir(200), Some(Dir::Left));

    // A direction tap is the explicit stop signal in classic mode.
    drain(&mut t, 201, &[('P', KeyCode::Left)]);
    assert_eq!(t.active_dir(201), None);
}

// ── Classic terminal: the tmux F/W-release bug ────────────────────────────────
// Reproduces the reported bug: glide ← / → , press F (fast move), release F
// while STILL holding the direction key.  In tmux nothing arrives after the
// F release — macOS auto-repeat never resumes for the direction key — so the
// old silence expiry stopped the ship even though the key was held.

#[test]
fn classic_f_release_glide_continues_until_dir_tap() {
    let mut t = classic();
    // Right held: OS repeat presses engage the glide.
    drain(&mut t, 10, &[('P', KeyCode::Right)]);
    for f in 11..=15 {
        drain(&mut t, f, &[('P', KeyCode::Right)]);
    }
    assert_eq!(t.active_dir(15), Some(Dir::Right));

    // F pressed mid-glide: OS repeat switches to F; Right goes silent.
    drain(&mut t, 16, &[('P', KeyCode::Char('f'))]);
    // F repeats after the OS initial delay.
    idle(&mut t, 17..=25);
    for f in 26..=35 {
        drain(&mut t, f, &[('P', KeyCode::Char('f'))]);
        assert_eq!(t.active_dir(f), Some(Dir::Right), "frame {}", f);
    }

    // F released, Right still physically held: TOTAL silence from here on.
    // The glide must continue — indefinitely, because no event can ever
    // distinguish "Right still held" from "Right released" in classic mode.
    idle(&mut t, 36..=300);
    assert_eq!(t.active_dir(300), Some(Dir::Right));

    // Explicit stop: a single (non-rapid) direction tap disengages.
    drain(&mut t, 301, &[('P', KeyCode::Right)]);
    assert_eq!(t.active_dir(301), None);
    // …and the tap's OS repeats re-engage a fresh glide as usual.
    drain(&mut t, 303, &[('P', KeyCode::Right)]);
    assert_eq!(t.active_dir(303), Some(Dir::Right));
}

#[test]
fn classic_fw_press_without_glide_does_not_engage_phantom() {
    let mut t = classic();
    // F/W tapped with no direction engaged: nothing to keep alive.
    drain(&mut t, 10, &[('P', KeyCode::Char('f'))]);
    drain(&mut t, 11, &[('P', KeyCode::Char('w'))]);
    idle(&mut t, 12..=40);
    assert_eq!(t.active_dir(40), None);
}

#[test]
fn classic_space_autofire_does_not_sustain_glide() {
    let mut t = classic();
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    drain(&mut t, 12, &[('P', KeyCode::Left)]);
    assert_eq!(t.active_dir(12), Some(Dir::Left));

    // Direction released; player keeps autofiring Space (OS repeat presses).
    // Space must NOT keep the ship sliding.
    for f in 13..=12 + HOLD_WINDOW {
        drain(&mut t, f, &[('P', KeyCode::Char(' '))]);
    }
    let f = 12 + HOLD_WINDOW + 1;
    drain(&mut t, f, &[('P', KeyCode::Char(' '))]);
    assert_eq!(t.active_dir(f), None);
}

// ── Enhanced terminal: true release stops instantly ──────────────────────────

#[test]
fn enhanced_true_release_stops_same_frame() {
    let mut t = enhanced();
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    drain(&mut t, 12, &[('T', KeyCode::Left)]);
    drain(&mut t, 13, &[('T', KeyCode::Left)]);
    assert_eq!(t.active_dir(13), Some(Dir::Left));

    // True release: no other key pressed in the same drain.
    drain(&mut t, 14, &[('X', KeyCode::Left)]);
    assert_eq!(t.active_dir(14), None);
}

#[test]
fn repeat_then_release_same_drain_stops() {
    let mut t = enhanced();
    drain(&mut t, 10, &[('T', KeyCode::Left)]);
    // Straggler Repeat and Release in the same flush: release wins.
    drain(&mut t, 11, &[('T', KeyCode::Left), ('X', KeyCode::Left)]);
    assert_eq!(t.active_dir(11), None);
}

#[test]
fn enhanced_release_while_holding_w_stops() {
    let mut t = enhanced();
    drain(&mut t, 10, &[('T', KeyCode::Left)]);
    drain(
        &mut t,
        11,
        &[('P', KeyCode::Char('w')), ('X', KeyCode::Left)],
    );
    // ↑ false release (W pressed same drain) → still moving.
    assert_eq!(t.active_dir(11), Some(Dir::Left));

    // Direction released for real while W repeats: the Release arrives in a
    // drain with no Press → true release → stop instantly.
    drain(
        &mut t,
        12,
        &[('T', KeyCode::Char('w')), ('X', KeyCode::Left)],
    );
    assert_eq!(t.active_dir(12), None);
}

// ── Enhanced terminal: F/W combos survive repeat suppression ─────────────────
// Ghostty stops sending direction-key Repeats while F/W is held, and fires a
// false Release for the direction key when F/W is pressed.  The combo must
// stay alive on F/W's own events.

#[test]
fn enhanced_f_hold_keeps_combo_alive() {
    let mut t = enhanced();
    drain(&mut t, 10, &[('T', KeyCode::Left)]);
    assert_eq!(t.active_dir(10), Some(Dir::Left));

    // F pressed → Ghostty fires a false Release for Left in the same drain
    // and stops Left repeats while F is held.
    drain(
        &mut t,
        11,
        &[('P', KeyCode::Char('f')), ('X', KeyCode::Left)],
    );
    assert_eq!(t.active_dir(11), Some(Dir::Left));

    // Long F hold: only F repeats arrive. Combo stays alive well past
    // HOLD_WINDOW (this was the regression).
    for f in 12..=11 + ENHANCED_SILENCE_LIMIT {
        drain(&mut t, f, &[('T', KeyCode::Char('f'))]);
        assert_eq!(t.active_dir(f), Some(Dir::Left), "frame {}", f);
    }

    // F released → Left repeats resume → still gliding.
    let f = 12 + ENHANCED_SILENCE_LIMIT;
    drain(&mut t, f, &[('X', KeyCode::Char('f'))]);
    drain(&mut t, f + 1, &[('T', KeyCode::Left)]);
    assert_eq!(t.active_dir(f + 1), Some(Dir::Left));

    // Left released for real → instant stop.
    drain(&mut t, f + 2, &[('X', KeyCode::Left)]);
    assert_eq!(t.active_dir(f + 2), None);
}

#[test]
fn false_release_during_space_tap_keeps_moving() {
    let mut t = enhanced();
    drain(&mut t, 20, &[('T', KeyCode::Left)]);
    assert_eq!(t.active_dir(20), Some(Dir::Left));

    // Space press + Left release in the same drain → false release.
    drain(
        &mut t,
        21,
        &[('P', KeyCode::Char(' ')), ('X', KeyCode::Left)],
    );
    assert_eq!(t.active_dir(21), Some(Dir::Left));

    // Repeats resume → still gliding.
    drain(&mut t, 22, &[('T', KeyCode::Left)]);
    assert_eq!(t.active_dir(22), Some(Dir::Left));
}

#[test]
fn enhanced_swallowed_true_release_bounded_by_safety_net() {
    let mut t = enhanced();
    drain(&mut t, 20, &[('T', KeyCode::Left)]);

    // User releases Left at the same instant as pressing Space: the release
    // is swallowed as "false", but no further movement events ever arrive.
    drain(
        &mut t,
        21,
        &[('P', KeyCode::Char(' ')), ('X', KeyCode::Left)],
    );
    assert_eq!(t.active_dir(21), Some(Dir::Left));

    // The slide is bounded by the enhanced silence safety net (~1 s).
    idle(&mut t, 22..=20 + ENHANCED_SILENCE_LIMIT + 1);
    assert_eq!(t.active_dir(20 + ENHANCED_SILENCE_LIMIT + 1), None);
}

// ── Direction switching & independence ────────────────────────────────────────

#[test]
fn opposite_direction_takes_over() {
    let mut t = enhanced();
    drain(&mut t, 10, &[('T', KeyCode::Left)]);
    assert_eq!(t.active_dir(10), Some(Dir::Left));
    drain(&mut t, 11, &[('T', KeyCode::Right)]);
    assert_eq!(t.active_dir(11), Some(Dir::Right));
}

#[test]
fn release_of_other_direction_does_not_stop_movement() {
    let mut t = enhanced();
    drain(&mut t, 10, &[('T', KeyCode::Left)]);
    drain(&mut t, 11, &[('T', KeyCode::Left), ('X', KeyCode::Right)]);
    assert_eq!(t.active_dir(11), Some(Dir::Left));
}

#[test]
fn non_direction_keys_never_produce_movement() {
    let mut t = classic();
    drain(&mut t, 10, &[('P', KeyCode::Char(' '))]);
    drain(&mut t, 11, &[('P', KeyCode::Char(' '))]);
    assert_eq!(t.active_dir(11), None);
}

#[test]
fn is_held_tracks_non_direction_keys() {
    let mut t = classic();
    drain(&mut t, 10, &[('P', KeyCode::Char('f'))]);
    assert!(t.is_held(&KeyCode::Char('f'), 10));
    assert!(t.is_held(&KeyCode::Char('f'), 10 + HOLD_WINDOW));
    assert!(!t.is_held(&KeyCode::Char('f'), 10 + HOLD_WINDOW + 1));
}

// ── Backward-compatibility lock ───────────────────────────────────────────────
// A rapid re-press reports `rapid = true` so the game loop suppresses the
// single-step move (continuous movement takes over); a fresh press reports
// false so move_player_left/right stay the single source of step movement.

#[test]
fn rapid_repress_reports_rapid() {
    let mut t = classic();
    t.begin_drain(10);
    assert!(!t.record_press(KeyCode::Left, 10));
    t.end_drain(10);
    t.begin_drain(12);
    assert!(t.record_press(KeyCode::Left, 12));
    t.end_drain(12);
}

#[test]
fn press_after_true_release_is_not_rapid() {
    let mut t = enhanced();
    drain(&mut t, 10, &[('P', KeyCode::Left)]);
    drain(&mut t, 11, &[('X', KeyCode::Left)]);
    t.begin_drain(12);
    // Released state cleared the Held timestamp → fresh single-step press.
    assert!(!t.record_press(KeyCode::Left, 12));
    t.end_drain(12);
}
