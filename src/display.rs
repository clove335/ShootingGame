/// Rendering layer — all terminal I/O lives here.

use std::io::Write;

use crossterm::{
    cursor,
    style::{self, Color, Print},
    terminal,
    QueueableCommand,
};
use shooting_game::entities::{
    BonusItem, BonusKind, Bullet, BulletOwner, Enemy, EnemyKind, EntireGameStateInfo,
    Explosion, FlameBullet, FirebombProj, GameStatus, Level,
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
const C_BONUS_SPREAD: Color = Color::Yellow;
const C_BONUS_LIFE: Color = Color::Magenta;
const C_BONUS_RAPID: Color = Color::Cyan;
const C_BONUS_FLAME: Color = Color::Rgb { r: 255, g: 128, b: 0 };  // orange
const C_BONUS_BOMB: Color = Color::DarkRed;
const C_FLAME_BULLET: Color = Color::Rgb { r: 255, g: 128, b: 0 }; // orange
const C_FIREBOMB: Color = Color::Red;
const C_EXPLOSION: Color = Color::Rgb { r: 255, g: 200, b: 0 };    // bright orange-yellow
const C_POWERUP_ACTIVE: Color = Color::Yellow;
const C_HINT: Color = Color::DarkGrey;

// ── Public entry point ────────────────────────────────────────────────────────

pub fn render<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    out.queue(terminal::Clear(terminal::ClearType::All))?;

    draw_border(out, state)?;
    draw_hud(out, state)?;

    for enemy in &state.enemies {
        draw_enemy(out, enemy, state.height as i32 - 2)?;
    }
    for bonus in &state.bonus_items {
        draw_bonus_item(out, bonus)?;
    }
    for exp in &state.explosions {
        draw_explosion(out, exp)?;
    }
    for fb in &state.flame_bullets {
        draw_flame_bullet(out, fb)?;
    }
    for bomb in &state.firebombs {
        draw_firebomb(out, bomb)?;
    }
    for bullet in &state.bullets {
        draw_bullet(out, bullet)?;
    }

    draw_player(out, state)?;
    draw_controls_hint(out, state)?;

    if state.status == GameStatus::GameOver {
        draw_game_over(out, state)?;
    }

    out.queue(style::ResetColor)?;
    out.queue(cursor::MoveTo(0, state.height.saturating_sub(1)))?;
    out.flush()?;
    Ok(())
}

// ── Border ────────────────────────────────────────────────────────────────────

fn draw_border<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    let w = state.width as usize;
    let h = state.height;
    out.queue(style::SetForegroundColor(C_BORDER))?;
    out.queue(cursor::MoveTo(0, 1))?;
    out.queue(Print(format!("┌{}┐", "─".repeat(w.saturating_sub(2)))))?;
    out.queue(cursor::MoveTo(0, h.saturating_sub(2)))?;
    out.queue(Print(format!("└{}┘", "─".repeat(w.saturating_sub(2)))))?;
    for row in 2..h.saturating_sub(2) {
        out.queue(cursor::MoveTo(0, row))?;
        out.queue(Print("│"))?;
        out.queue(cursor::MoveTo(state.width.saturating_sub(1), row))?;
        out.queue(Print("│"))?;
    }
    Ok(())
}

// ── HUD ───────────────────────────────────────────────────────────────────────

fn draw_hud<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    // Score + high score — left
    out.queue(cursor::MoveTo(1, 0))?;
    out.queue(style::SetForegroundColor(C_HUD_SCORE))?;
    if state.high_score > 0 {
        out.queue(Print(format!("Score:{:>6}  Hi:{:>6}", state.score, state.high_score)))?;
    } else {
        out.queue(Print(format!("Score:{:>6}", state.score)))?;
    }

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

    // Active power-up indicator + lives — right
    let power_tag = match &state.active_power_up {
        Some((BonusKind::SpreadShot, f)) => format!("[★ SPREAD {:>2}s] ", f / 30 + 1),
        Some((BonusKind::RapidFire,  f)) => format!("[! RAPID  {:>2}s] ", f / 30 + 1),
        Some((BonusKind::FlameBurst, f)) => format!("[~ FLAME  {:>2}s] ", f / 30 + 1),
        Some((BonusKind::Firebomb,   f)) => format!("[o BOMB   {:>2}s] ", f / 30 + 1),
        _ => String::new(),
    };
    let hearts = "♥".repeat(state.player.lives as usize);
    let lives_str = format!("Lives:{}", hearts);
    let right_str = format!("{}{}", power_tag, lives_str);
    let rx = state.width.saturating_sub(right_str.chars().count() as u16 + 1);
    out.queue(cursor::MoveTo(rx, 0))?;
    if !power_tag.is_empty() {
        out.queue(style::SetForegroundColor(C_POWERUP_ACTIVE))?;
        out.queue(Print(&power_tag))?;
    }
    out.queue(style::SetForegroundColor(C_HUD_LIVES))?;
    out.queue(Print(&lives_str))?;

    Ok(())
}

// ── Entities ──────────────────────────────────────────────────────────────────

fn draw_player<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    let p = &state.player;
    out.queue(style::SetForegroundColor(C_PLAYER))?;
    out.queue(cursor::MoveTo(p.x as u16, p.y as u16))?;
    out.queue(Print("▲"))?;
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
    play_bottom: i32,
) -> std::io::Result<()> {
    let lx = (enemy.x - 1).max(0) as u16;
    match enemy.kind {
        EnemyKind::Spacecraft => {
            out.queue(style::SetForegroundColor(C_ENEMY_SPACECRAFT))?;
            out.queue(cursor::MoveTo(lx, enemy.y as u16))?;
            out.queue(Print("«▼»"))?;
            if enemy.y + 1 < play_bottom {
                out.queue(cursor::MoveTo(lx, (enemy.y + 1) as u16))?;
                out.queue(Print("╚═╝"))?;
            }
        }
        EnemyKind::Octopus => {
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
    out.queue(cursor::MoveTo(bullet.x as u16, bullet.y as u16))?;
    match bullet.owner {
        BulletOwner::Player => {
            out.queue(style::SetForegroundColor(C_BULLET_PLAYER))?;
            out.queue(Print("║"))?;
        }
        BulletOwner::Enemy => {
            out.queue(style::SetForegroundColor(C_BULLET_ENEMY))?;
            out.queue(Print("↓"))?;
        }
    }
    Ok(())
}

/// Draw one flame bullet. The displayed character hints at its angle:
///
/// ```text
///  vx ≤ −0.7  →  ╱   (steep left)
///  vx ≤ −0.1  →  /   (gentle left)
///  |vx| < 0.1 →  ~   (near-vertical)
///  vx ≥  0.1  →  \   (gentle right)
///  vx ≥  0.7  →  ╲   (steep right)
/// ```
fn draw_flame_bullet<W: Write>(out: &mut W, fb: &FlameBullet) -> std::io::Result<()> {
    let x = fb.x.round() as u16;
    let y = fb.y.round() as u16;
    out.queue(cursor::MoveTo(x, y))?;
    out.queue(style::SetForegroundColor(C_FLAME_BULLET))?;
    let ch = if fb.vx <= -0.7 {
        "╱"
    } else if fb.vx <= -0.1 {
        "/"
    } else if fb.vx < 0.1 {
        "~"
    } else if fb.vx < 0.7 {
        "\\"
    } else {
        "╲"
    };
    out.queue(Print(ch))?;
    Ok(())
}

/// Draw a firebomb in transit as a pulsing red circle.
fn draw_firebomb<W: Write>(out: &mut W, bomb: &FirebombProj) -> std::io::Result<()> {
    out.queue(cursor::MoveTo(bomb.x as u16, bomb.y as u16))?;
    // Alternate between ● and ○ based on fuse parity for a pulsing effect.
    let ch = if bomb.fuse % 6 < 3 { "●" } else { "○" };
    out.queue(style::SetForegroundColor(C_FIREBOMB))?;
    out.queue(Print(ch))?;
    Ok(())
}

/// Draw a firebomb explosion — a bright diamond of `*` characters.
///
/// Display radius 3 (Euclidean), slightly smaller than the kill radius (4)
/// so players can see enemies die "just outside" the visible blast.
fn draw_explosion<W: Write>(out: &mut W, exp: &Explosion) -> std::io::Result<()> {
    const R: i32 = 3;
    out.queue(style::SetForegroundColor(C_EXPLOSION))?;
    for dy in -R..=R {
        for dx in -R..=R {
            if dx * dx + dy * dy <= R * R {
                let px = exp.x + dx;
                let py = exp.y + dy;
                if px > 0 && py > 1 {
                    out.queue(cursor::MoveTo(px as u16, py as u16))?;
                    out.queue(Print("*"))?;
                }
            }
        }
    }
    Ok(())
}

/// Draw a falling bonus item.
///
/// | Symbol | Colour  | Power-up                                        |
/// |--------|---------|--------------------------------------------------|
/// | `★`    | Yellow  | SpreadShot — 3-way spread fire                  |
/// | `♥`    | Magenta | ExtraLife — restores one life                   |
/// | `!`    | Cyan    | RapidFire — 6-bullet cap                        |
/// | `~`    | Orange  | FlameBurst — 4-way 36° angled flame shots       |
/// | `o`    | DarkRed | Firebomb — slow explosive with area damage       |
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
        BonusKind::FlameBurst => {
            out.queue(style::SetForegroundColor(C_BONUS_FLAME))?;
            out.queue(Print("~"))?;
        }
        BonusKind::Firebomb => {
            out.queue(style::SetForegroundColor(C_BONUS_BOMB))?;
            out.queue(Print("o"))?;
        }
    }
    Ok(())
}

// ── Controls hint ─────────────────────────────────────────────────────────────

fn draw_controls_hint<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    out.queue(cursor::MoveTo(1, state.height.saturating_sub(1)))?;
    out.queue(style::SetForegroundColor(C_HINT))?;
    out.queue(Print("← → / A D : Move   SPACE : Shoot   Q : Quit"))?;
    Ok(())
}

// ── Game-over overlay ─────────────────────────────────────────────────────────

fn draw_game_over<W: Write>(out: &mut W, state: &EntireGameStateInfo) -> std::io::Result<()> {
    let score_line = format!("Final Score: {:>6}", state.score);
    let best_score = state.high_score.max(state.score);
    let is_new_best = state.score > 0 && state.score >= state.high_score;
    let best_line = if is_new_best {
        format!("★ NEW BEST: {:>6} ★", best_score)
    } else {
        format!("Best Score:  {:>6}", best_score)
    };

    let cx = state.width / 2;
    let lines: &[(&str, Color)] = &[
        ("╔════════════════════╗", Color::Red),
        ("║    GAME  OVER      ║", Color::Red),
        ("╚════════════════════╝", Color::Red),
    ];
    let start_row = (state.height / 2).saturating_sub((lines.len() + 3) as u16 / 2);

    for (i, (msg, color)) in lines.iter().enumerate() {
        let col = cx.saturating_sub(msg.chars().count() as u16 / 2);
        out.queue(cursor::MoveTo(col, start_row + i as u16))?;
        out.queue(style::SetForegroundColor(*color))?;
        out.queue(Print(*msg))?;
    }

    let score_row = start_row + lines.len() as u16;
    out.queue(cursor::MoveTo(cx.saturating_sub(score_line.len() as u16 / 2), score_row))?;
    out.queue(style::SetForegroundColor(Color::Yellow))?;
    out.queue(Print(&score_line))?;

    let best_row = score_row + 1;
    let best_color = if is_new_best { Color::Yellow } else { Color::DarkGrey };
    out.queue(cursor::MoveTo(cx.saturating_sub(best_line.chars().count() as u16 / 2), best_row))?;
    out.queue(style::SetForegroundColor(best_color))?;
    out.queue(Print(&best_line))?;

    let hint = "R - Play Again  Q - Quit";
    out.queue(cursor::MoveTo(cx.saturating_sub(hint.len() as u16 / 2), best_row + 1))?;
    out.queue(style::SetForegroundColor(Color::White))?;
    out.queue(Print(hint))?;

    Ok(())
}
