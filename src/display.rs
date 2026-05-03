//! Rendering layer — all terminal I/O lives here.
//!
//! Each function receives a mutable writer and an immutable view of the
//! game state.  No game logic is performed; this module only translates
//! state into terminal commands.

use std::io::Write;

use crate::entities::{
    BonusItem, BonusKind, Bullet, BulletOwner, Enemy, EnemyKind, EntireGameStateInfo, GameStatus,
    Level,
};
use crossterm::{
    cursor,
    style::{self, Color, Print},
    terminal, QueueableCommand,
};

// ── Colour palette ────────────────────────────────────────────────────────────

const C_BORDER: Color = Color::DarkBlue;
const C_HUD_SCORE: Color = Color::Yellow;
const C_HUD_LIVES: Color = Color::Red;
const C_PLAYER: Color = Color::White;
const C_ENEMY_SPACECRAFT: Color = Color::Green;
const C_ENEMY_OCTOPUS: Color = Color::Red;
const C_BULLET_PLAYER: Color = Color::Cyan;
const C_BULLET_ENEMY: Color = Color::Magenta;
const C_HINT: Color = Color::DarkGrey;
const C_BONUS_SPREAD: Color = Color::Yellow;
const C_BONUS_LIFE: Color = Color::Magenta;
const C_BONUS_RAPID: Color = Color::Cyan;
const C_POWERUP_ACTIVE: Color = Color::Yellow;

// ── Public entry point ────────────────────────────────────────────────────────

/// ## Why `full_redraw`?
///
/// `terminal::Clear(All)` repaints the entire terminal viewport every frame,
/// causing visible flicker and forcing the GPU to process every cell — even
/// the border and controls hint that never change.
///
/// On the **first frame** (`full_redraw = true`) we still do a full clear so
/// the screen starts clean, then draw the border and controls hint.
///
/// On every **subsequent frame** (`full_redraw = false`) we only erase:
/// - Row 0 (HUD) — score, timer, and lives change every frame.
/// - Rows 2 → h−3 (play area) — where all moving entities live.
///
/// The border (rows 1, h−2) and controls hint (row h−1) are **left in place**:
/// game entities are clamped inside the play area so they can never overwrite
/// those rows.  This cuts the terminal's per-frame work from O(viewport) to
/// O(play area), eliminating flicker on static regions entirely.
pub fn render<W: Write>(
    out: &mut W,
    state: &EntireGameStateInfo,
    full_redraw: bool,
) -> std::io::Result<()> {
    let w = state.width;
    let h = state.height;

    if full_redraw {
        // First frame: clear everything and paint the static chrome.
        out.queue(terminal::Clear(terminal::ClearType::All))?;
        draw_border(out, state)?;
        draw_controls_hint(out, state)?;
    } else {
        // Subsequent frames: erase only the two dynamic regions.

        // Row 0 — HUD (score, power-up countdown, lives all change each frame)
        out.queue(cursor::MoveTo(0, 0))?;
        out.queue(Print(" ".repeat(w as usize)))?;

        // Rows 2 … h-3 — clear play area and redraw side walls each frame.
        // Redrawing walls prevents ghost sprites when entities move through
        // col 0 or col w-1 (outside the cols-1..w-2 blank region).
        let blank = " ".repeat(w.saturating_sub(2) as usize);
        for row in 2u16..h.saturating_sub(2) {
            out.queue(style::SetForegroundColor(C_BORDER))?;
            out.queue(cursor::MoveTo(0, row))?;
            out.queue(Print("│"))?;
            out.queue(style::ResetColor)?;
            out.queue(cursor::MoveTo(1, row))?;
            out.queue(Print(&blank))?;
            out.queue(style::SetForegroundColor(C_BORDER))?;
            out.queue(cursor::MoveTo(w.saturating_sub(1), row))?;
            out.queue(Print("│"))?;
        }
    }

    // Always repaint dynamic content.
    draw_hud(out, state)?;

    for enemy in &state.enemies {
        draw_enemy(out, enemy, h as i32 - 2)?;
    }
    for bonus in &state.bonus_items {
        draw_bonus_item(out, bonus)?;
    }
    for bullet in &state.bullets {
        draw_bullet(out, bullet)?;
    }
    draw_player(out, state)?;

    if let Some((msg, _)) = &state.cheer_msg {
        draw_cheer(out, state, msg)?;
    }

    if state.status == GameStatus::GameOver {
        draw_game_over(out, state)?;
    }

    if state.debug_mode {
        draw_debug_overlay(out, state)?;
    }

    // Park cursor in a harmless spot and flush
    out.queue(style::ResetColor)?;
    out.queue(cursor::MoveTo(0, h.saturating_sub(1)))?;
    out.flush()?;
    Ok(())
}

// ── Border ────────────────────────────────────────────────────────────────────

fn draw_border<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
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

fn draw_hud<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    // Score and high score — left
    out.queue(cursor::MoveTo(1, 0))?;
    out.queue(style::SetForegroundColor(C_HUD_SCORE))?;
    if state.high_score > 0 {
        out.queue(Print(format!(
            "Score:{:>6}  Hi:{:>6}",
            state.score, state.high_score
        )))?;
    } else {
        out.queue(Print(format!("Score:{:>6}", state.score)))?;
    }

    // Level — centre
    let level_str = match state.level {
        Level::Easy => "[ EASY ]",
        Level::Medium => "[ MEDIUM ]",
        Level::Hard => "[ HARD ]",
        Level::Extreme => "[ EXTREME ]",
    };
    let level_color = match state.level {
        Level::Easy => Color::Green,
        Level::Medium => Color::Yellow,
        Level::Hard => Color::Red,
        Level::Extreme => Color::Magenta,
    };
    let lx = (state.width / 2).saturating_sub(level_str.len() as u16 / 2);
    out.queue(cursor::MoveTo(lx, 0))?;
    out.queue(style::SetForegroundColor(level_color))?;
    out.queue(Print(level_str))?;

    // Active power-up indicator + lives — right side
    // Build the right-side string, right-aligned
    let power_tag = match &state.active_power_up {
        Some((BonusKind::SpreadShot, frames)) => {
            format!("[★ SPREAD {:>2}s] ", frames / 30 + 1)
        }
        Some((BonusKind::RapidFire, frames)) => {
            format!("[! RAPID  {:>2}s] ", frames / 30 + 1)
        }
        _ => String::new(),
    };
    let is_rapid = matches!(&state.active_power_up, Some((BonusKind::RapidFire, _)));
    let bullet_cap = if is_rapid { 6 } else { 3 };
    let active_bullets = state
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .count();
    let bullet_slots: String = (0..bullet_cap)
        .map(|i| if i < active_bullets { '●' } else { '○' })
        .collect();
    let bullet_str = format!("[{}] ", bullet_slots);

    let hearts: String = "♥".repeat(state.player.lives as usize);
    let lives_str = format!("Lives:{}", hearts);
    let right_str = format!("{}{}{}", power_tag, bullet_str, lives_str);

    let rx = state
        .width
        .saturating_sub(right_str.chars().count() as u16 + 1);
    out.queue(cursor::MoveTo(rx, 0))?;

    // Colour the power-up tag separately if present
    if !power_tag.is_empty() {
        out.queue(style::SetForegroundColor(C_POWERUP_ACTIVE))?;
        out.queue(Print(&power_tag))?;
    }
    // Bullet slots: cyan when slots available, red when full
    let slot_color = if active_bullets >= bullet_cap {
        Color::Red
    } else {
        Color::Cyan
    };
    out.queue(style::SetForegroundColor(slot_color))?;
    out.queue(Print(&bullet_str))?;
    out.queue(style::SetForegroundColor(C_HUD_LIVES))?;
    out.queue(Print(&lives_str))?;

    Ok(())
}

// ── Entities ──────────────────────────────────────────────────────────────────

fn draw_player<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    // Enhanced sprite (2 rows, 3 cols):
    //   ▲       ← row y      (tip)
    //  /█\      ← row y+1    (fuselage + wings)
    let p = &state.player;
    let flashing = state.muzzle_flash > 0;

    // Muzzle flash: bright burst one row above the tip
    if flashing {
        let flash_y = p.y - 1;
        if flash_y >= 2 {
            out.queue(style::SetForegroundColor(Color::Yellow))?;
            out.queue(cursor::MoveTo(p.x as u16, flash_y as u16))?;
            out.queue(Print("*"))?;
        }
    }

    // Tip — yellow while firing, white otherwise
    let tip_color = if flashing { Color::Yellow } else { C_PLAYER };
    out.queue(style::SetForegroundColor(tip_color))?;
    out.queue(cursor::MoveTo(p.x as u16, p.y as u16))?;
    out.queue(Print("▲"))?;

    // Fuselage — starting one column left of centre
    out.queue(style::SetForegroundColor(C_PLAYER))?;
    let wing_y = p.y + 1;
    if wing_y < state.height as i32 - 2 {
        out.queue(cursor::MoveTo((p.x - 1).max(1) as u16, wing_y as u16))?;
        out.queue(Print("/█\\"))?;
    }

    Ok(())
}

fn draw_enemy<W: Write>(
    out: &mut W,
    enemy: &Enemy,
    play_bottom: i32, // bottom border row (= height - 2)
) -> std::io::Result<()> {
    let lx = (enemy.x - 1).max(0) as u16;
    match enemy.kind {
        EnemyKind::Spacecraft => {
            // Enhanced sprite:
            //   «▼»    ← swept-back wings
            //   ╚═╝    ← engine block
            out.queue(style::SetForegroundColor(C_ENEMY_SPACECRAFT))?;
            out.queue(cursor::MoveTo(lx, enemy.y as u16))?;
            out.queue(Print("«▼»"))?;
            if enemy.y + 1 < play_bottom {
                out.queue(cursor::MoveTo(lx, (enemy.y + 1) as u16))?;
                out.queue(Print("╚═╝"))?;
            }
        }
        EnemyKind::Octopus => {
            // Enhanced sprite:
            //   (◎)    ← glowing eye
            //   ╰─╯    ← tentacle arc
            out.queue(style::SetForegroundColor(C_ENEMY_OCTOPUS))?;
            out.queue(cursor::MoveTo(lx, enemy.y as u16))?;
            out.queue(Print("(◎)"))?;
            if enemy.y + 1 < play_bottom {
                out.queue(cursor::MoveTo(lx, (enemy.y + 1) as u16))?;
                out.queue(Print("╰─╯"))?;
            }
        }
    }
    Ok(())
}

fn draw_bullet<W: Write>(out: &mut W, bullet: &Bullet) -> std::io::Result<()> {
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

/// Draw a falling bonus item.
///
/// Symbols:
///   ★  (yellow)  — SpreadShot: collect for 3-way spread fire
///   ♥  (magenta) — ExtraLife:  instantly restores one life
///   !  (cyan)    — RapidFire:  raises the bullet cap to 6
fn draw_bonus_item<W: Write>(out: &mut W, bonus: &BonusItem) -> std::io::Result<()> {
    out.queue(cursor::MoveTo(bonus.x as u16, bonus.y as u16))?;
    match bonus.kind {
        BonusKind::SpreadShot => {
            out.queue(style::SetForegroundColor(C_BONUS_SPREAD))?;
            out.queue(Print("★"))?;
        }
        BonusKind::ExtraLife => {
            out.queue(style::SetForegroundColor(C_BONUS_LIFE))?;
            out.queue(Print("♥"))?;
        }
        BonusKind::RapidFire => {
            out.queue(style::SetForegroundColor(C_BONUS_RAPID))?;
            out.queue(Print("!"))?;
        }
    }
    Ok(())
}

// ── Controls hint (last row) ──────────────────────────────────────────────────

fn draw_controls_hint<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    out.queue(cursor::MoveTo(1, state.height.saturating_sub(1)))?;
    out.queue(style::SetForegroundColor(C_HINT))?;
    out.queue(Print("← → / A D : Move   SPACE : Shoot   Q : Quit"))?;
    Ok(())
}

// ── Score-milestone cheer ─────────────────────────────────────────────────────

fn draw_cheer<W: Write>(
    out: &mut W,
    state: &EntireGameStateInfo,
    msg: &str,
) -> std::io::Result<()> {
    let cx = state.width / 2;
    let row = 2u16 + (state.height.saturating_sub(4)) / 4;
    let col = cx.saturating_sub(msg.chars().count() as u16 / 2);
    out.queue(cursor::MoveTo(col, row))?;
    out.queue(style::SetForegroundColor(Color::Cyan))?;
    out.queue(Print(msg))?;
    Ok(())
}

// ── Debug overlay ────────────────────────────────────────────────────────────

fn draw_debug_overlay<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    let player_bullets = state
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .count();
    let enemy_bullets = state.bullets.len() - player_bullets;

    let pu = match &state.active_power_up {
        Some((BonusKind::SpreadShot, f)) => format!("Spread({}f)", f),
        Some((BonusKind::RapidFire, f)) => format!("Rapid({}f)", f),
        Some((BonusKind::ExtraLife, _)) => "ExtraLife".to_string(),
        None => "-".to_string(),
    };
    let god = if state.god_mode { "ON" } else { "OFF" };
    let slow = if state.slow_mo { "ON" } else { "OFF" };

    let lines = [
        format!(
            " F:{:<6} P:({},{})  E:{:<3} B:{}p+{}e",
            state.frame,
            state.player.x,
            state.player.y,
            state.enemies.len(),
            player_bullets,
            enemy_bullets
        ),
        format!(" PU:{:<18} GOD:{}  SLOW:{}", pu, god, slow),
    ];

    for (i, line) in lines.iter().enumerate() {
        out.queue(cursor::MoveTo(0, 2 + i as u16))?;
        out.queue(style::SetForegroundColor(Color::DarkGrey))?;
        out.queue(Print("█"))?;
        out.queue(style::SetForegroundColor(Color::White))?;
        out.queue(Print(line))?;
        out.queue(style::ResetColor)?;
    }

    // Collision boxes
    draw_hitbox(out, state.player.x, state.player.y, Color::Cyan)?;
    for enemy in &state.enemies {
        draw_hitbox(out, enemy.x, enemy.y, Color::Red)?;
    }

    Ok(())
}

/// Draw a 3-wide × 2-tall bounding box around (cx, top_y).
fn draw_hitbox<W: Write>(out: &mut W, cx: i32, top_y: i32, color: Color) -> std::io::Result<()> {
    let corners = [
        (cx - 1, top_y),
        (cx + 1, top_y),
        (cx - 1, top_y + 1),
        (cx + 1, top_y + 1),
    ];
    out.queue(style::SetForegroundColor(color))?;
    for (x, y) in corners {
        if x >= 0 && y >= 0 {
            out.queue(cursor::MoveTo(x as u16, y as u16))?;
            out.queue(Print("·"))?;
        }
    }
    out.queue(style::ResetColor)?;
    Ok(())
}

// ── Game-over overlay ─────────────────────────────────────────────────────────

fn draw_game_over<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    let score_line = format!("Final Score: {:>6}", state.score);
    let best_score = state.high_score.max(state.score);
    let best_line = if state.score >= state.high_score && state.score > 0 {
        format!("★ NEW BEST: {:>6} ★", best_score)
    } else {
        format!("Best Score:  {:>6}", best_score)
    };

    let lines: &[(&str, Color)] = &[
        ("╔════════════════════╗", Color::Red),
        ("║    GAME  OVER      ║", Color::Red),
        ("╚════════════════════╝", Color::Red),
    ];
    let score_color = Color::Yellow;
    let best_color = if state.score >= state.high_score && state.score > 0 {
        Color::Yellow
    } else {
        Color::DarkGrey
    };
    let hint_color = Color::White;

    let cx = state.width / 2;
    let total_rows = lines.len() + 3; // 3 box lines + score + best + hint
    let start_row = (state.height / 2).saturating_sub(total_rows as u16 / 2);

    for (i, (msg, color)) in lines.iter().enumerate() {
        let row = start_row + i as u16;
        let col = cx.saturating_sub(msg.chars().count() as u16 / 2);
        out.queue(cursor::MoveTo(col, row))?;
        out.queue(style::SetForegroundColor(*color))?;
        out.queue(Print(*msg))?;
    }

    let score_row = start_row + lines.len() as u16;
    let col = cx.saturating_sub(score_line.chars().count() as u16 / 2);
    out.queue(cursor::MoveTo(col, score_row))?;
    out.queue(style::SetForegroundColor(score_color))?;
    out.queue(Print(&score_line))?;

    let best_row = score_row + 1;
    let col = cx.saturating_sub(best_line.chars().count() as u16 / 2);
    out.queue(cursor::MoveTo(col, best_row))?;
    out.queue(style::SetForegroundColor(best_color))?;
    out.queue(Print(&best_line))?;

    let hint = "R - Play Again  Q - Quit";
    let hint_row = best_row + 1;
    let col = cx.saturating_sub(hint.chars().count() as u16 / 2);
    out.queue(cursor::MoveTo(col, hint_row))?;
    out.queue(style::SetForegroundColor(hint_color))?;
    out.queue(Print(hint))?;

    Ok(())
}
