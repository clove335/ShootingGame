mod display;

use std::collections::HashMap;
use std::io::{stdout, BufWriter, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{
        self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    style::{self, Color, Print},
    terminal, ExecutableCommand, QueueableCommand,
};
use rand::thread_rng;

use shooting_game::compute::{init_state, move_player_left, move_player_right, player_shoot, tick};
use shooting_game::entities::{EntireGameStateInfo, GameStatus, Level};

const FRAME: Duration = Duration::from_millis(33); // ≈30 FPS

// ── Simultaneous-input constants ──────────────────────────────────────────────

/// Min frames between player movements while a direction key is held.
/// 1.3 frames @ 30 FPS ≈ 23 moves/sec.
const MOVE_COOLDOWN: f64 = 1.3;

/// A key is considered "held" if its last press/repeat event arrived within
/// this many frames — bridges the OS key-repeat initial delay (~7–15 frames).
const HOLD_WINDOW: u64 = 20;

/// Frames to keep a key alive after a Release before truly stopping.
///
/// Ghostty (Kitty keyboard protocol) fires a Release for the held movement key
/// the moment a second key (e.g. Space) is pressed, and also stops sending
/// Repeat for that key while the second key is held.  Without a grace period
/// the movement key expires immediately.  6 frames (~200 ms) covers a quick
/// Space tap; if Space is held longer the player briefly stops then resumes.
const GRACE_PERIOD: u64 = 6;

/// Returns true if `key` is currently held.
fn is_held(
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
                    last_press >= last_release
                        || frame.saturating_sub(last_release) <= GRACE_PERIOD
                }
            }
        }
    }
}

// ── High-score persistence ────────────────────────────────────────────────────

fn high_score_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".shooting_game_score")
}

fn load_high_score() -> u32 {
    std::fs::read_to_string(high_score_path())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn save_high_score(score: u32) {
    let _ = std::fs::write(high_score_path(), score.to_string());
}

// ── Menu ──────────────────────────────────────────────────────────────────────

enum MenuResult {
    Start(Level),
    Quit,
}

fn show_menu<W: Write>(
    out: &mut W,
    rx: &mpsc::Receiver<Event>,
    high_score: u32,
) -> std::io::Result<MenuResult> {
    out.queue(terminal::Clear(terminal::ClearType::All))?;

    let (width, height) = terminal::size()?;
    let cx = width / 2;
    let cy = height / 2;

    let title = "★  SPACE  SHOOTER  ★";
    out.queue(cursor::MoveTo(
        cx.saturating_sub(title.chars().count() as u16 / 2),
        cy.saturating_sub(6),
    ))?;
    out.queue(style::SetForegroundColor(Color::Cyan))?;
    out.queue(Print(title))?;

    // High score display
    if high_score > 0 {
        let hs_str = format!("Best Score: {}", high_score);
        out.queue(cursor::MoveTo(
            cx.saturating_sub(hs_str.chars().count() as u16 / 2),
            cy.saturating_sub(5),
        ))?;
        out.queue(style::SetForegroundColor(Color::Yellow))?;
        out.queue(Print(&hs_str))?;
    }

    out.queue(cursor::MoveTo(cx.saturating_sub(10), cy.saturating_sub(3)))?;
    out.queue(style::SetForegroundColor(Color::White))?;
    out.queue(Print("Select difficulty:"))?;

    let options: &[(&str, &str, Color, &str)] = &[
        ("1", "Easy   ", Color::Green, "Very slow enemies, relaxed pace"),
        ("2", "Medium ", Color::Yellow, "Balanced challenge"),
        ("3", "Hard   ", Color::Red, "Fast and relentless!"),
        ("4", "Extreme", Color::Magenta, "Unforgiving — good luck"),
    ];

    for (i, (key, label, color, desc)) in options.iter().enumerate() {
        let row = cy.saturating_sub(1) + i as u16;
        out.queue(cursor::MoveTo(cx.saturating_sub(10), row))?;
        out.queue(style::SetForegroundColor(Color::DarkGrey))?;
        out.queue(Print(format!("[{}] ", key)))?;
        out.queue(style::SetForegroundColor(*color))?;
        out.queue(Print(format!("{:<8}", label)))?;
        out.queue(style::SetForegroundColor(Color::DarkGrey))?;
        out.queue(Print(format!(" — {}", desc)))?;
    }

    // Bonus item legend
    out.queue(cursor::MoveTo(cx.saturating_sub(10), cy + 3))?;
    out.queue(style::SetForegroundColor(Color::DarkGrey))?;
    out.queue(Print("Power-ups (catch falling items):"))?;

    let bonus_info: &[(&str, Color, &str)] = &[
        ("★", Color::Yellow, " SpreadShot — 3-way fire"),
        ("♥", Color::Magenta, " ExtraLife  — +1 life"),
        ("!", Color::Cyan, " RapidFire  — 6 bullets on screen"),
    ];
    for (i, (sym, color, desc)) in bonus_info.iter().enumerate() {
        let row = cy + 4 + i as u16;
        out.queue(cursor::MoveTo(cx.saturating_sub(10), row))?;
        out.queue(style::SetForegroundColor(*color))?;
        out.queue(Print(sym))?;
        out.queue(style::SetForegroundColor(Color::DarkGrey))?;
        out.queue(Print(*desc))?;
    }

    out.queue(cursor::MoveTo(cx.saturating_sub(10), cy + 8))?;
    out.queue(style::SetForegroundColor(Color::DarkGrey))?;
    out.queue(Print("← → / A D : Move   SPACE : Shoot   Q : Quit"))?;

    out.queue(style::ResetColor)?;
    out.flush()?;

    // Block until the user makes a choice
    loop {
        if let Ok(Event::Key(KeyEvent { code, .. })) = rx.recv() {
            match code {
                KeyCode::Char('1') => return Ok(MenuResult::Start(Level::Easy)),
                KeyCode::Char('2') => return Ok(MenuResult::Start(Level::Medium)),
                KeyCode::Char('3') => return Ok(MenuResult::Start(Level::Hard)),
                KeyCode::Char('4') => return Ok(MenuResult::Start(Level::Extreme)),
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                    return Ok(MenuResult::Quit);
                }
                _ => {}
            }
        }
    }
}

// ── Game loop ─────────────────────────────────────────────────────────────────

/// Returns `true` → quit program,  `false` → back to menu.
///
/// Input model: instead of acting on each key event individually, we maintain
/// a `key_frame` map that records the frame number of the last press/repeat
/// event for every key.  Each frame we check which keys are still "fresh"
/// (within `HOLD_WINDOW` frames) and apply all their effects simultaneously.
/// This allows Space + A/D to be held at the same time with no interference.
///
/// Works on two classes of terminal:
/// * **Keyboard-enhancement capable** (Ghostty, kitty, etc.): proper
///   `Press` / `Repeat` / `Release` events → keys are removed on release.
/// * **Classic terminals**: only `Press` events (OS key-repeat shows as
///   repeated `Press`).  Keys expire naturally after `HOLD_WINDOW` frames of
///   silence, which is shorter than the OS repeat interval, so the key stays
///   live while it is actively generating repeats.
fn game_loop<W: Write>(
    out: &mut W,
    state: &mut EntireGameStateInfo,
    rx: &mpsc::Receiver<Event>,
) -> std::io::Result<bool> {
    let mut rng = thread_rng();

    let mut key_frame: HashMap<KeyCode, u64> = HashMap::new();
    let mut release_frame: HashMap<KeyCode, u64> = HashMap::new();
    let mut move_cooldown: f64 = 0.0;
    let mut frame: u64 = 0;
    let mut first_frame = true;

    loop {
        let frame_start = Instant::now();
        frame += 1;

        // ── Drain all pending input events (non-blocking) ─────────────────────
        while let Ok(Event::Key(KeyEvent {
            code,
            kind,
            modifiers,
            ..
        })) = rx.try_recv()
        {
            match kind {
                // Press: record key + handle one-shot actions
                KeyEventKind::Press => {
                    match code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            return Ok(true);
                        }
                        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(true);
                        }
                        KeyCode::Char('r') | KeyCode::Char('R')
                            if state.status == GameStatus::GameOver =>
                        {
                            return Ok(false);
                        }
                        // Space: single-shot on press — not tracked in key_frame so it
                        // never interferes with held movement keys.
                        KeyCode::Char(' ') if state.status == GameStatus::Playing => {
                            *state = player_shoot(state);
                        }
                        _ => {
                            key_frame.insert(code, frame);
                        }
                    }
                }
                // Repeat: refresh timestamp so key stays "held"
                KeyEventKind::Repeat => {
                    key_frame.insert(code, frame);
                }
                // Release: record the release frame instead of removing from
                // key_frame directly — a subsequent Repeat (same or next frame)
                // will set key_frame[key] >= release_frame[key], keeping the
                // key live and preventing false stops from chord detection.
                KeyEventKind::Release => {
                    release_frame.insert(code, frame);
                }
            }
        }

        // ── Apply held-key actions every frame ────────────────────────────────
        if state.status == GameStatus::Playing {
            let left = is_held(&key_frame, &release_frame, &KeyCode::Left, frame)
                || is_held(&key_frame, &release_frame, &KeyCode::Char('a'), frame)
                || is_held(&key_frame, &release_frame, &KeyCode::Char('A'), frame);
            let right = is_held(&key_frame, &release_frame, &KeyCode::Right, frame)
                || is_held(&key_frame, &release_frame, &KeyCode::Char('d'), frame)
                || is_held(&key_frame, &release_frame, &KeyCode::Char('D'), frame);

            if move_cooldown <= 0.0 {
                if left {
                    *state = move_player_left(state);
                    move_cooldown = MOVE_COOLDOWN;
                } else if right {
                    *state = move_player_right(state);
                    move_cooldown = MOVE_COOLDOWN;
                }
            }
        }

        move_cooldown = (move_cooldown - 1.0).max(0.0);

        if state.status == GameStatus::Playing {
            *state = tick(state, &mut rng);
        }

        display::render(out, state, first_frame)?;
        first_frame = false;

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME {
            std::thread::sleep(FRAME - elapsed);
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> std::io::Result<()> {
    let raw_out = stdout();
    let mut out = BufWriter::new(raw_out);

    terminal::enable_raw_mode()?;
    out.execute(terminal::EnterAlternateScreen)?;
    out.execute(cursor::Hide)?;

    // Request key-release (and key-repeat) events from the terminal.
    // Ghostty / kitty-protocol terminals support this; others fall back gracefully.
    let keyboard_enhanced = out
        .execute(PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_EVENT_TYPES,
        ))
        .is_ok();

    // Dedicate a thread exclusively to blocking event reads, sending them
    // through a channel so the game loop never has to block on I/O.
    let (tx, rx) = mpsc::channel::<Event>();
    thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if tx.send(ev).is_err() {
                break; // receiver dropped → program exiting
            }
        }
    });

    let result = run(&mut out, &rx);

    // Always restore the terminal
    if keyboard_enhanced {
        let _ = out.execute(PopKeyboardEnhancementFlags);
    }
    let _ = out.execute(cursor::Show);
    let _ = out.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();

    result
}

fn run<W: Write>(out: &mut W, rx: &mpsc::Receiver<Event>) -> std::io::Result<()> {
    let mut high_score = load_high_score();

    loop {
        match show_menu(out, rx, high_score)? {
            MenuResult::Quit => break,
            MenuResult::Start(level) => {
                let (width, height) = terminal::size()?;
                let mut state = init_state(level, width, height, high_score);
                let quit = game_loop(out, &mut state, rx)?;

                // Persist new high score if beaten
                if state.score > high_score {
                    high_score = state.score;
                    save_high_score(high_score);
                }

                if quit {
                    break;
                }
                // Otherwise loop back to the menu
            }
        }
    }
    Ok(())
}
