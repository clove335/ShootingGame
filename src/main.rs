mod compute;
mod display;
mod entities;

use std::io::{stdout, BufWriter, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{self, Color, Print},
    terminal,
    ExecutableCommand, QueueableCommand,
};
use rand::thread_rng;

use compute::{init_state, move_player_left, move_player_right, player_shoot, tick};
use entities::{GameState, GameStatus, Level};

const FRAME: Duration = Duration::from_millis(33); // ≈30 FPS

// ── Menu ──────────────────────────────────────────────────────────────────────

enum MenuResult {
    Start(Level),
    Quit,
}

fn show_menu<W: Write>(out: &mut W) -> std::io::Result<MenuResult> {
    out.queue(terminal::Clear(terminal::ClearType::All))?;

    let (width, height) = terminal::size()?;
    let cx = width / 2;
    let cy = height / 2;

    // Title
    let title = "★  SPACE  SHOOTER  ★";
    out.queue(cursor::MoveTo(
        cx.saturating_sub(title.chars().count() as u16 / 2),
        cy.saturating_sub(6),
    ))?;
    out.queue(style::SetForegroundColor(Color::Cyan))?;
    out.queue(Print(title))?;

    // Subtitle
    out.queue(cursor::MoveTo(cx.saturating_sub(10), cy.saturating_sub(4)))?;
    out.queue(style::SetForegroundColor(Color::White))?;
    out.queue(Print("Select difficulty:"))?;

    let options: &[(&str, &str, Color, &str)] = &[
        ("1", "Easy  ", Color::Green,  "Slow enemies, relaxed pace"),
        ("2", "Medium", Color::Yellow, "Balanced challenge"),
        ("3", "Hard  ", Color::Red,    "Fast and relentless!"),
    ];

    for (i, (key, label, color, desc)) in options.iter().enumerate() {
        let row = cy.saturating_sub(2) + i as u16;
        out.queue(cursor::MoveTo(cx.saturating_sub(10), row))?;
        out.queue(style::SetForegroundColor(Color::DarkGrey))?;
        out.queue(Print(format!("[{}] ", key)))?;
        out.queue(style::SetForegroundColor(*color))?;
        out.queue(Print(format!("{:<8}", label)))?;
        out.queue(style::SetForegroundColor(Color::DarkGrey))?;
        out.queue(Print(format!(" — {}", desc)))?;
    }

    // Controls preview
    out.queue(cursor::MoveTo(cx.saturating_sub(10), cy + 2))?;
    out.queue(style::SetForegroundColor(Color::DarkGrey))?;
    out.queue(Print("In-game:  ← → / A D  Move   SPACE  Shoot   Q  Quit"))?;

    out.queue(cursor::MoveTo(cx.saturating_sub(10), cy + 4))?;
    out.queue(style::SetForegroundColor(Color::DarkGrey))?;
    out.queue(Print("[Q]  Quit"))?;

    out.queue(style::ResetColor)?;
    out.flush()?;

    loop {
        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Char('1') => return Ok(MenuResult::Start(Level::Easy)),
                KeyCode::Char('2') => return Ok(MenuResult::Start(Level::Medium)),
                KeyCode::Char('3') => return Ok(MenuResult::Start(Level::Hard)),
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                    return Ok(MenuResult::Quit)
                }
                _ => {}
            }
        }
    }
}

// ── Game loop ─────────────────────────────────────────────────────────────────

/// Returns `true` when the player wants to quit, `false` when they want to
/// restart (go back to the menu).
fn game_loop<W: Write>(out: &mut W, state: &mut GameState) -> std::io::Result<bool> {
    let mut rng = thread_rng();

    loop {
        let frame_start = Instant::now();

        // Handle all pending key events (non-blocking)
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read()?
            {
                match code {
                    KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                        *state = move_player_left(state);
                    }
                    KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                        *state = move_player_right(state);
                    }
                    KeyCode::Char(' ') => {
                        *state = player_shoot(state);
                    }
                    KeyCode::Char('r') | KeyCode::Char('R')
                        if state.status == GameStatus::GameOver =>
                    {
                        return Ok(false); // back to menu
                    }
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                        return Ok(true);
                    }
                    KeyCode::Char('c')
                        if modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        return Ok(true);
                    }
                    _ => {}
                }
            }
        }

        if state.status == GameStatus::Playing {
            *state = tick(state, &mut rng);
        }

        display::render(out, state)?;

        // Sleep for the remainder of the frame budget
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

    let result = run(&mut out);

    // Always restore the terminal, even on error
    let _ = out.execute(cursor::Show);
    let _ = out.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();

    result
}

fn run<W: Write>(out: &mut W) -> std::io::Result<()> {
    loop {
        match show_menu(out)? {
            MenuResult::Quit => break,
            MenuResult::Start(level) => {
                let (width, height) = terminal::size()?;
                let mut state = init_state(level, width, height);
                let quit = game_loop(out, &mut state)?;
                if quit {
                    break;
                }
                // Otherwise loop back to the menu
            }
        }
    }
    Ok(())
}
