mod db;

use shooting_game::display;

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

use shooting_game::compute::{
    init_state, move_player_left, move_player_left_n, move_player_right, move_player_right_n,
    player_shoot, tick,
};
use shooting_game::entities::{EntireGameStateInfo, GameStatus, Level};
use shooting_game::input_keyboard::{Dir, InputTracker};

const FRAME: Duration = Duration::from_millis(33); // ≈30 FPS

// ── Simultaneous-input constants ──────────────────────────────────────────────

/// Min frames between player movements while a direction key is held.
/// 1.0 resets to 0 after one decrement → player moves every frame (30 cols/sec).
const MOVE_COOLDOWN: f64 = 0.1;
/// Frames between warp jumps while W is held (≈3–4 warps/sec at 30 FPS).
const WARP_COOLDOWN: f64 = 8.0;

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
        (
            "~",
            Color::Rgb {
                r: 255,
                g: 128,
                b: 0,
            },
            " FlameBurst — 4-way angled fire",
        ),
        ("o", Color::DarkRed, " Firebomb   — slow bomb, area blast"),
    ];
    for (i, (sym, color, desc)) in bonus_info.iter().enumerate() {
        let row = cy + 4 + i as u16;
        out.queue(cursor::MoveTo(cx.saturating_sub(10), row))?;
        out.queue(style::SetForegroundColor(*color))?;
        out.queue(Print(sym))?;
        out.queue(style::SetForegroundColor(Color::DarkGrey))?;
        out.queue(Print(*desc))?;
    }

    out.queue(cursor::MoveTo(cx.saturating_sub(10), cy + 10))?;
    out.queue(style::SetForegroundColor(Color::DarkGrey))?;
    out.queue(Print(
        "← → / A D : Move   F+dir : Fast   W+dir : Warp×10   SPACE : Shoot   Q : Quit",
    ))?;

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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn dbg_key(code: KeyCode) -> &'static str {
    match code {
        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => "<",
        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => ">",
        KeyCode::Char(' ') => "_",
        KeyCode::Char('f') | KeyCode::Char('F') => "F",
        KeyCode::Char('w') | KeyCode::Char('W') => "W",
        _ => "?",
    }
}

// ── Game loop ─────────────────────────────────────────────────────────────────

/// Returns `true` → quit program,  `false` → back to menu.
///
/// Input model: all key state lives in an [`InputTracker`] (see
/// `input_keyboard.rs`).  Each frame we drain pending events into the tracker,
/// then query `active_dir()` / `is_held()` to apply movement.  `enhanced`
/// reflects whether the terminal actually delivers kitty-protocol Release
/// events (raw Ghostty: yes; inside tmux: no) and selects the tracker's
/// trust model — see `InputTracker` docs.
fn game_loop<W: Write>(
    out: &mut W,
    state: &mut EntireGameStateInfo,
    rx: &mpsc::Receiver<Event>,
    demo_mode: bool,
    enhanced: bool,
) -> std::io::Result<bool> {
    let mut rng = thread_rng();

    let mut tracker = InputTracker::new(enhanced);
    let mut move_cooldown: f64 = 0.0;
    let mut warp_cooldown: f64 = 0.0;
    let mut frame: u64 = 0;
    let mut first_frame = true;

    loop {
        let frame_start = Instant::now();
        frame += 1;

        // ── Drain all pending input events (non-blocking) ─────────────────────
        tracker.begin_drain(frame);
        // Compact event log: "P<" = Press Left, "T<" = Repeat Left, "X<" = Release Left.
        let mut dbg_ev = String::new();

        while let Ok(Event::Key(KeyEvent {
            code,
            kind,
            modifiers,
            ..
        })) = rx.try_recv()
        {
            match kind {
                // Press: record into tracker + handle one-shot actions.
                KeyEventKind::Press => {
                    if state.debug_mode {
                        dbg_ev.push_str(&format!("P{} ", dbg_key(code)));
                    }
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
                        _ => {}
                    }

                    // `rapid` = second press of a direction key within RAPID_WINDOW
                    // (classic-terminal key-repeat): continuous movement takes over,
                    // so no single-step move is applied for it.
                    let rapid = tracker.record_press(code, frame);

                    if state.status == GameStatus::Playing {
                        match code {
                            KeyCode::Char(' ') => {
                                *state = player_shoot(state);
                            }
                            // Movement keys: move one step immediately on press.
                            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') if !rapid => {
                                *state = move_player_left(state);
                            }
                            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') if !rapid => {
                                *state = move_player_right(state);
                            }
                            // W: instant warp 10 steps on keydown (if direction active).
                            KeyCode::Char('w') | KeyCode::Char('W') => {
                                match tracker.active_dir(frame) {
                                    Some(Dir::Left) => {
                                        *state = move_player_left_n(state, 10);
                                        warp_cooldown = WARP_COOLDOWN;
                                    }
                                    Some(Dir::Right) => {
                                        *state = move_player_right_n(state, 10);
                                        warp_cooldown = WARP_COOLDOWN;
                                    }
                                    None => {}
                                }
                            }
                            // F: instant 2-step move on keydown (if direction active).
                            KeyCode::Char('f') | KeyCode::Char('F') => {
                                match tracker.active_dir(frame) {
                                    Some(Dir::Left) => *state = move_player_left_n(state, 2),
                                    Some(Dir::Right) => *state = move_player_right_n(state, 2),
                                    None => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
                KeyEventKind::Repeat => {
                    if state.debug_mode {
                        dbg_ev.push_str(&format!("T{} ", dbg_key(code)));
                    }
                    tracker.record_repeat(code, frame);
                }
                KeyEventKind::Release => {
                    if state.debug_mode {
                        dbg_ev.push_str(&format!("X{} ", dbg_key(code)));
                    }
                    tracker.record_release(code);
                }
            }
        }

        // Apply deferred releases (false-release detection lives in the tracker).
        tracker.end_drain(frame);

        // ── Input diagnostics (debug overlay) ────────────────────────────────
        if state.debug_mode {
            let fmt_dir = |d: Option<Dir>| match d {
                Some(Dir::Left) => "L",
                Some(Dir::Right) => "R",
                None => "-",
            };
            state.debug_input = format!(
                " kbd:{} raw:{} act:{} ndp:{} ev:[{}]",
                if enhanced { "ENH" } else { "CLS" },
                fmt_dir(tracker.raw_dir()),
                fmt_dir(tracker.active_dir(frame)),
                tracker.non_dir_press_seen() as u8,
                dbg_ev.trim_end(),
            );
        }

        // ── Apply Demo Mode actions ───────────────────────────────────────────
        if demo_mode {
            if state.status == GameStatus::GameOver {
                return Ok(false); // auto-restart: run() will loop back
            }
            if state.status == GameStatus::Playing {
                *state = shooting_game::demo::update_demo(state);
            }
        }

        // ── Apply held-key actions every frame ────────────────────────────────
        if state.status == GameStatus::Playing {
            let fast = tracker.is_held(&KeyCode::Char('f'), frame)
                || tracker.is_held(&KeyCode::Char('F'), frame);
            let warp = tracker.is_held(&KeyCode::Char('w'), frame)
                || tracker.is_held(&KeyCode::Char('W'), frame);

            // active_dir() = engaged direction AND fresh key events within
            // HOLD_WINDOW. On terminals that never send Release (e.g. tmux with
            // extended-keys off) the freshness gate is the only stop signal.
            let dir = tracker.active_dir(frame);

            if warp && warp_cooldown <= 0.0 {
                match dir {
                    Some(Dir::Left) => {
                        *state = move_player_left_n(state, 10);
                        warp_cooldown = WARP_COOLDOWN;
                    }
                    Some(Dir::Right) => {
                        *state = move_player_right_n(state, 10);
                        warp_cooldown = WARP_COOLDOWN;
                    }
                    None => {}
                }
            } else if fast {
                match dir {
                    Some(Dir::Left) => *state = move_player_left_n(state, 2),
                    Some(Dir::Right) => *state = move_player_right_n(state, 2),
                    None => {}
                }
            } else if move_cooldown <= 0.0 {
                match dir {
                    Some(Dir::Left) => {
                        *state = move_player_left(state);
                        move_cooldown = MOVE_COOLDOWN;
                    }
                    Some(Dir::Right) => {
                        *state = move_player_right(state);
                        move_cooldown = MOVE_COOLDOWN;
                    }
                    None => {}
                }
            }
        }

        move_cooldown = (move_cooldown - 1.0).max(0.0);
        warp_cooldown = (warp_cooldown - 1.0).max(0.0);

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
    let demo_mode = std::env::args().any(|arg| arg == "--demo-play");

    let raw_out = stdout();
    let mut out = BufWriter::new(raw_out);

    terminal::enable_raw_mode()?;

    // Query whether the terminal really supports the kitty keyboard protocol
    // (Release/Repeat events).  Writing the push sequence alone proves nothing:
    // tmux silently swallows it, so `.execute().is_ok()` is always true even
    // when no Release event will ever arrive.  The query must happen before
    // the event-reader thread spawns, because it reads the tty response itself.
    let keyboard_enhanced = terminal::supports_keyboard_enhancement().unwrap_or(false);

    out.execute(terminal::EnterAlternateScreen)?;
    out.execute(cursor::Hide)?;

    if keyboard_enhanced {
        out.execute(PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_EVENT_TYPES,
        ))?;
    }

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

    let result = run(&mut out, &rx, demo_mode, keyboard_enhanced);

    // Always restore the terminal
    if keyboard_enhanced {
        let _ = out.execute(PopKeyboardEnhancementFlags);
    }
    let _ = out.execute(cursor::Show);
    let _ = out.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();

    result
}

fn run<W: Write>(
    out: &mut W,
    rx: &mpsc::Receiver<Event>,
    demo_mode: bool,
    enhanced: bool,
) -> std::io::Result<()> {
    let username = std::env::var("USER").unwrap_or_else(|_| "Player".to_string());
    let db_conn = db::open();
    let mut high_score = db_conn.as_ref().map_or(0, db::load_best_score);

    loop {
        let menu_res = if demo_mode {
            MenuResult::Start(Level::Hard)
        } else {
            show_menu(out, rx, high_score)?
        };

        match menu_res {
            MenuResult::Quit => break,
            MenuResult::Start(level) => {
                let difficulty_best = db_conn
                    .as_ref()
                    .map_or(0, |c| db::load_top_score(c, &level));
                let (width, height) = terminal::size()?;
                let mut state = init_state(level, width, height, difficulty_best);
                let quit = game_loop(out, &mut state, rx, demo_mode, enhanced)?;

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

                if demo_mode {
                    // Small delay before restarting AI play
                    std::thread::sleep(Duration::from_millis(1500));
                }
            }
        }
    }
    Ok(())
}
