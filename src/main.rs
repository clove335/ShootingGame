mod db;

use shooting_game::display;

use std::collections::HashMap;
use std::io::{stdout, BufWriter, Write};
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
use shooting_game::input_keyboard::is_held;

const FRAME: Duration = Duration::from_millis(33); // ≈30 FPS

// ── Simultaneous-input constants ──────────────────────────────────────────────

/// Min frames between player movements while a direction key is held.
/// 1.0 resets to 0 after one decrement → player moves every frame (30 cols/sec).
const MOVE_COOLDOWN: f64 = 0.1;

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
        (
            "1",
            "Easy   ",
            Color::Green,
            "Very slow enemies, relaxed pace",
        ),
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
    // True once a Repeat event (or a rapid-fire os-simulated Press) has arrived
    // for each direction since the last genuine Press.  Continuous movement only
    // fires when this is true, so a tap (Press + Release, no Repeat) stays one step.
    let mut left_has_repeat = false;
    let mut right_has_repeat = false;
    let mut frame: u64 = 0;
    let mut first_frame = true;

    loop {
        let frame_start = Instant::now();
        frame += 1;

        // ── Drain all pending input events (non-blocking) ─────────────────────
        // Release events are deferred to the end of the drain so that a straggler
        // Repeat arriving in the same OS-queue flush cannot re-enable a key that
        // was just released.
        let mut deferred_releases: Vec<KeyCode> = Vec::new();

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
                        // Movement keys: move one step immediately on press.
                        // For classic terminals the OS sends repeated Press events
                        // instead of Repeat; treat a rapid second Press (within 4
                        // frames) as a Repeat so continuous movement still works.
                        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A')
                            if state.status == GameStatus::Playing =>
                        {
                            let rapid = key_frame
                                .get(&code)
                                .is_some_and(|&last| frame.saturating_sub(last) <= 4);
                            if rapid {
                                left_has_repeat = true;
                            } else {
                                *state = move_player_left(state);
                                left_has_repeat = false;
                            }
                            key_frame.insert(code, frame);
                        }
                        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D')
                            if state.status == GameStatus::Playing =>
                        {
                            let rapid = key_frame
                                .get(&code)
                                .is_some_and(|&last| frame.saturating_sub(last) <= 4);
                            if rapid {
                                right_has_repeat = true;
                            } else {
                                *state = move_player_right(state);
                                right_has_repeat = false;
                            }
                            key_frame.insert(code, frame);
                        }
                        // Backtick: toggle debug overlay.
                        KeyCode::Char('`') => {
                            state.debug_mode = !state.debug_mode;
                        }
                        // G: toggle god mode (only while debug is on).
                        KeyCode::Char('g') | KeyCode::Char('G') if state.debug_mode => {
                            state.god_mode = !state.god_mode;
                        }
                        // S: toggle slow-mo (only while debug is on).
                        KeyCode::Char('s') | KeyCode::Char('S') if state.debug_mode => {
                            state.slow_mo = !state.slow_mo;
                        }
                        _ => {
                            key_frame.insert(code, frame);
                        }
                    }
                }
                // Repeat: refresh timestamp and mark direction as held.
                KeyEventKind::Repeat => {
                    match code {
                        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                            left_has_repeat = true;
                        }
                        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                            right_has_repeat = true;
                        }
                        _ => {}
                    }
                    key_frame.insert(code, frame);
                }
                // Release: defer until all Press/Repeat events this frame are handled.
                KeyEventKind::Release => {
                    deferred_releases.push(code);
                }
            }
        }

        // Apply deferred releases — runs after all Repeat events this cycle.
        for code in deferred_releases {
            match code {
                KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                    left_has_repeat = false;
                }
                KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                    right_has_repeat = false;
                }
                _ => {}
            }
            release_frame.insert(code, frame);
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
                if left && left_has_repeat {
                    *state = move_player_left(state);
                    move_cooldown = MOVE_COOLDOWN;
                } else if right && right_has_repeat {
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

        let target = if state.slow_mo { FRAME * 4 } else { FRAME };
        let elapsed = frame_start.elapsed();
        if elapsed < target {
            std::thread::sleep(target - elapsed);
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
    let username = std::env::var("USER").unwrap_or_else(|_| "Player".to_string());
    let db_conn = db::open();
    let mut high_score = db_conn.as_ref().map_or(0, db::load_best_score);

    loop {
        match show_menu(out, rx, high_score)? {
            MenuResult::Quit => break,
            MenuResult::Start(level) => {
                let difficulty_best = db_conn
                    .as_ref()
                    .map_or(0, |c| db::load_top_score(c, &level));
                let (width, height) = terminal::size()?;
                let mut state = init_state(level, width, height, difficulty_best);
                let quit = game_loop(out, &mut state, rx)?;

                if state.status == GameStatus::GameOver {
                    if let Some(ref conn) = db_conn {
                        let _ = db::insert_score(conn, &username, &state.level, state.score);
                    }
                }

                if let Some(ref conn) = db_conn {
                    let _ = db::upsert_top_score(conn, &username, &state.level, state.score);
                }

                if state.score > high_score {
                    high_score = state.score;
                }

                if quit {
                    break;
                }
            }
        }
    }
    Ok(())
}
