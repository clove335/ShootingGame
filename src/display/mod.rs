/// Rendering layer — all terminal I/O lives here.
///
/// Each function receives a mutable writer and an immutable view of the
/// game state.  No game logic is performed; this module only translates
/// state into terminal commands.

use std::io::Write;

use crossterm::{
    cursor,
    style::{self, Color, Print},
    terminal,
    QueueableCommand,
};
use crate::entities::{BulletOwner, EnemyKind, GameState, GameStatus, Level};

// ── Colour palette ────────────────────────────────────────────────────────────

const C_BORDER: Color = Color::DarkBlue;
const C_HUD_SCORE: Color = Color::Yellow;
const C_HUD_LIVES: Color = Color::Red;
const C_PLAYER: Color = Color::White;
const C_ENEMY_SPACECRAFT: Color = Color::Green;   // "shocking green"
const C_ENEMY_OCTOPUS: Color = Color::Red;
const C_BULLET_PLAYER: Color = Color::Cyan;
const C_BULLET_ENEMY: Color = Color::Magenta;
const C_HINT: Color = Color::DarkGrey;

// ── Public entry point ────────────────────────────────────────────────────────

/// Render one complete frame.
pub fn render<W: Write>(out: &mut W, state: &GameState) -> std::io::Result<()> {
    out.queue(terminal::Clear(terminal::ClearType::All))?;

    draw_border(out, state)?;
    draw_hud(out, state)?;

    for enemy in &state.enemies {
        draw_enemy(out, enemy, state.height as i32 - 2)?;
    }
    for bullet in &state.bullets {
        draw_bullet(out, bullet)?;
    }

    draw_player(out, state)?;
    draw_controls_hint(out, state)?;

    if state.status == GameStatus::GameOver {
        draw_game_over(out, state)?;
    }

    // Park cursor in a harmless spot and flush
    out.queue(style::ResetColor)?;
    out.queue(cursor::MoveTo(0, state.height.saturating_sub(1)))?;
    out.flush()?;
    Ok(())
}

// ── Border ────────────────────────────────────────────────────────────────────

fn draw_border<W: Write>(out: &mut W, state: &GameState) -> std::io::Result<()> {
    let w = state.width as usize;
    let h = state.height;

    out.queue(style::SetForegroundColor(C_BORDER))?;

    // Row 1 — top bar
    out.queue(cursor::MoveTo(0, 1))?;
    out.queue(Print(format!("┌{}┐", "─".repeat(w.saturating_sub(2)))))?;

    // Row h-2 — bottom bar
    out.queue(cursor::MoveTo(0, h.saturating_sub(2)))?;
    out.queue(Print(format!("└{}┘", "─".repeat(w.saturating_sub(2)))))?;

    // Side walls
    for row in 2..h.saturating_sub(2) {
        out.queue(cursor::MoveTo(0, row))?;
        out.queue(Print("│"))?;
        out.queue(cursor::MoveTo(state.width.saturating_sub(1), row))?;
        out.queue(Print("│"))?;
    }

    Ok(())
}

// ── HUD (row 0) ───────────────────────────────────────────────────────────────

fn draw_hud<W: Write>(out: &mut W, state: &GameState) -> std::io::Result<()> {
    // Score — left
    out.queue(cursor::MoveTo(1, 0))?;
    out.queue(style::SetForegroundColor(C_HUD_SCORE))?;
    out.queue(Print(format!("Score: {:>8}", state.score)))?;

    // Level — centre
    let level_str = match state.level {
        Level::Easy => "[ EASY ]",
        Level::Medium => "[ MEDIUM ]",
        Level::Hard => "[ HARD ]",
    };
    let level_color = match state.level {
        Level::Easy => Color::Green,
        Level::Medium => Color::Yellow,
        Level::Hard => Color::Red,
    };
    let lx = (state.width / 2).saturating_sub(level_str.len() as u16 / 2);
    out.queue(cursor::MoveTo(lx, 0))?;
    out.queue(style::SetForegroundColor(level_color))?;
    out.queue(Print(level_str))?;

    // Lives — right
    let hearts: String = "♥".repeat(state.player.lives as usize);
    let lives_text = format!("Lives: {}", hearts);
    let rx = state
        .width
        .saturating_sub(lives_text.chars().count() as u16 + 1);
    out.queue(cursor::MoveTo(rx, 0))?;
    out.queue(style::SetForegroundColor(C_HUD_LIVES))?;
    out.queue(Print(&lives_text))?;

    Ok(())
}

// ── Entities ──────────────────────────────────────────────────────────────────

fn draw_player<W: Write>(out: &mut W, state: &GameState) -> std::io::Result<()> {
    // Sprite (2 rows, 3 cols):
    //   ▲       ← row y      (tip)
    //  /|\      ← row y+1    (wings + fuselage)
    let p = &state.player;
    out.queue(style::SetForegroundColor(C_PLAYER))?;

    // Tip
    out.queue(cursor::MoveTo(p.x as u16, p.y as u16))?;
    out.queue(Print("▲"))?;

    // Wings — print "/|\" as a single 3-char string starting one col left
    let wing_y = p.y + 1;
    if wing_y < state.height as i32 - 2 {
        out.queue(cursor::MoveTo((p.x - 1).max(1) as u16, wing_y as u16))?;
        out.queue(Print("/|\\"))?;
    }

    Ok(())
}

fn draw_enemy<W: Write>(
    out: &mut W,
    enemy: &crate::entities::Enemy,
    play_bottom: i32, // bottom border row (= height - 2)
) -> std::io::Result<()> {
    let lx = (enemy.x - 1).max(0) as u16;
    match enemy.kind {
        EnemyKind::Spacecraft => {
            // Row 0:  <▼>
            // Row 1:  [_]
            out.queue(style::SetForegroundColor(C_ENEMY_SPACECRAFT))?;
            out.queue(cursor::MoveTo(lx, enemy.y as u16))?;
            out.queue(Print("<▼>"))?;
            if enemy.y + 1 < play_bottom {
                out.queue(cursor::MoveTo(lx, (enemy.y + 1) as u16))?;
                out.queue(Print("[_]"))?;
            }
        }
        EnemyKind::Octopus => {
            // Row 0:  (◉)
            // Row 1:  \-/
            out.queue(style::SetForegroundColor(C_ENEMY_OCTOPUS))?;
            out.queue(cursor::MoveTo(lx, enemy.y as u16))?;
            out.queue(Print("(◉)"))?;
            if enemy.y + 1 < play_bottom {
                out.queue(cursor::MoveTo(lx, (enemy.y + 1) as u16))?;
                out.queue(Print("\\-/"))?;
            }
        }
    }
    Ok(())
}

fn draw_bullet<W: Write>(
    out: &mut W,
    bullet: &crate::entities::Bullet,
) -> std::io::Result<()> {
    match bullet.owner {
        BulletOwner::Player => {
            out.queue(cursor::MoveTo(bullet.x as u16, bullet.y as u16))?;
            out.queue(style::SetForegroundColor(C_BULLET_PLAYER))?;
            out.queue(Print("║"))?;
        }
        BulletOwner::Enemy => {
            out.queue(cursor::MoveTo(bullet.x as u16, bullet.y as u16))?;
            out.queue(style::SetForegroundColor(C_BULLET_ENEMY))?;
            out.queue(Print("↓"))?;
        }
    }
    Ok(())
}

// ── Controls hint (last row) ──────────────────────────────────────────────────

fn draw_controls_hint<W: Write>(out: &mut W, state: &GameState) -> std::io::Result<()> {
    out.queue(cursor::MoveTo(1, state.height.saturating_sub(1)))?;
    out.queue(style::SetForegroundColor(C_HINT))?;
    out.queue(Print("← → / A D : Move   SPACE : Shoot   Q : Quit"))?;
    Ok(())
}

// ── Game-over overlay ─────────────────────────────────────────────────────────

fn draw_game_over<W: Write>(out: &mut W, state: &GameState) -> std::io::Result<()> {
    let score_line = format!("Final Score: {}", state.score);
    let lines: &[(&str, Color)] = &[
        ("╔══════════════════╗", Color::Red),
        ("║    GAME  OVER    ║", Color::Red),
        ("╚══════════════════╝", Color::Red),
        (&score_line,            Color::Yellow),
        ("R - Play Again  Q - Quit", Color::White),
    ];

    let cx = state.width / 2;
    let start_row = (state.height / 2).saturating_sub(lines.len() as u16 / 2);

    for (i, (msg, color)) in lines.iter().enumerate() {
        let row = start_row + i as u16;
        let col = cx.saturating_sub(msg.chars().count() as u16 / 2);
        out.queue(cursor::MoveTo(col, row))?;
        out.queue(style::SetForegroundColor(*color))?;
        out.queue(Print(*msg))?;
    }

    Ok(())
}
