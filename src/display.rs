//! Rendering layer — all terminal I/O lives here.
//!
//! Each function receives a mutable writer and an immutable view of the
//! game state.  No game logic is performed; this module only translates
//! state into terminal commands.

use std::io::Write;

use crossterm::{
    cursor,
    style::{self, Color, Print},
    terminal, QueueableCommand,
};
use shooting_game::entities::{
    BonusItem, BonusKind, Bullet, BulletOwner, Enemy, EnemyKind, EntireGameStateInfo, GameStatus,
    Level,
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

        // Rows 2 … h-3 — clear the full row width (including col 0 and col w-1)
        // so entities that rendered at the border columns (e.g. enemy lx=0 or
        // bullet at x=w-1) leave no ghost.  Restore the wall glyphs afterward.
        let blank = " ".repeat(w as usize);
        for row in 2u16..h.saturating_sub(2) {
            out.queue(cursor::MoveTo(0, row))?;
            out.queue(Print(&blank))?;
            out.queue(style::SetForegroundColor(C_BORDER))?;
            out.queue(cursor::MoveTo(0, row))?;
            out.queue(Print("│"))?;
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

    if state.status == GameStatus::GameOver {
        draw_game_over(out, state)?;
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
    let hearts: String = "♥".repeat(state.player.lives as usize);
    let lives_str = format!("Lives:{}", hearts);
    let right_str = format!("{}{}", power_tag, lives_str);

    let rx = state
        .width
        .saturating_sub(right_str.chars().count() as u16 + 1);
    out.queue(cursor::MoveTo(rx, 0))?;

    // Colour the power-up tag separately if present
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
    // Enhanced sprite (2 rows, 3 cols):
    //   ▲       ← row y      (tip)
    //  /█\      ← row y+1    (fuselage + wings)
    let p = &state.player;
    out.queue(style::SetForegroundColor(C_PLAYER))?;

    // Tip
    out.queue(cursor::MoveTo(p.x as u16, p.y as u16))?;
    out.queue(Print("▲"))?;

    // Fuselage — starting one column left of centre
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use shooting_game::entities::{
        Bullet, BulletOwner, Enemy, EnemyKind, EntireGameStateInfo, GameStatus, Level, Player,
    };

    // ── Virtual terminal emulator ─────────────────────────────────────────────
    //
    // Parses the raw escape-sequence bytes that crossterm writes into a `Vec<u8>`
    // and maintains a 2-D character grid — just like a real terminal would.
    // Supported sequences (the only ones render() uses):
    //   ESC [ <row> ; <col> H  — MoveTo (1-indexed)
    //   ESC [ 2 J              — Clear All
    //   ESC [ … m              — color/reset (ignored; we only check content)

    struct VirtualScreen {
        grid: Vec<Vec<char>>,
        col: usize,
        row: usize,
        width: usize,
        height: usize,
    }

    impl VirtualScreen {
        fn new(width: usize, height: usize) -> Self {
            Self {
                grid: vec![vec![' '; width]; height],
                col: 0,
                row: 0,
                width,
                height,
            }
        }

        /// Apply one frame's worth of escape-sequence output to the screen.
        fn apply(&mut self, buf: &[u8]) {
            let s = std::str::from_utf8(buf).expect("crossterm output must be valid UTF-8");
            let bytes = s.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                    // ESC[ — find the final byte (letter)
                    let start = i + 2;
                    let mut j = start;
                    while j < bytes.len()
                        && (bytes[j].is_ascii_digit() || bytes[j] == b';' || bytes[j] == b'?')
                    {
                        j += 1;
                    }
                    if j < bytes.len() {
                        let params = &s[start..j];
                        match bytes[j] {
                            b'H' => {
                                // Cursor position: ESC[row;colH (1-indexed)
                                let mut parts =
                                    params.split(';').filter_map(|p| p.parse::<usize>().ok());
                                self.row = parts
                                    .next()
                                    .unwrap_or(1)
                                    .saturating_sub(1)
                                    .min(self.height.saturating_sub(1));
                                self.col = parts
                                    .next()
                                    .unwrap_or(1)
                                    .saturating_sub(1)
                                    .min(self.width.saturating_sub(1));
                            }
                            b'J' if params == "2" => {
                                for r in self.grid.iter_mut() {
                                    r.fill(' ');
                                }
                            }
                            _ => {} // colors etc. — not needed for content checks
                        }
                        i = j + 1;
                    } else {
                        i += 1;
                    }
                } else {
                    let ch = s[i..].chars().next().unwrap();
                    if !ch.is_control() {
                        if self.row < self.height && self.col < self.width {
                            self.grid[self.row][self.col] = ch;
                        }
                        self.col += 1;
                    }
                    i += ch.len_utf8();
                }
            }
        }

        fn char_at(&self, col: usize, row: usize) -> char {
            self.grid[row][col]
        }

        fn row_str(&self, row: usize) -> String {
            self.grid[row].iter().collect()
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    const W: usize = 40;
    const H: usize = 20;

    fn make_state(width: u16, height: u16) -> EntireGameStateInfo {
        EntireGameStateInfo {
            player: Player {
                x: (width / 2) as i32,
                y: (height - 4) as i32,
                lives: 3,
            },
            enemies: vec![],
            bullets: vec![],
            bonus_items: vec![],
            active_power_up: None,
            score: 0,
            high_score: 0,
            level: Level::Easy,
            status: GameStatus::Playing,
            frame: 0,
            width,
            height,
        }
    }

    fn one_frame(state: &EntireGameStateInfo, full_redraw: bool) -> VirtualScreen {
        let mut buf = Vec::new();
        render(&mut buf, state, full_redraw).expect("render failed");
        let mut screen = VirtualScreen::new(state.width as usize, state.height as usize);
        screen.apply(&buf);
        screen
    }

    // ── Border tests ──────────────────────────────────────────────────────────

    #[test]
    fn first_frame_draws_top_border() {
        let state = make_state(W as u16, H as u16);
        let screen = one_frame(&state, true);
        assert_eq!(screen.char_at(0, 1), '┌', "top-left corner missing");
        assert_eq!(screen.char_at(W - 1, 1), '┐', "top-right corner missing");
        assert_eq!(screen.char_at(W / 2, 1), '─', "top border bar missing");
    }

    #[test]
    fn first_frame_draws_bottom_border() {
        let state = make_state(W as u16, H as u16);
        let screen = one_frame(&state, true);
        assert_eq!(screen.char_at(0, H - 2), '└', "bottom-left corner missing");
        assert_eq!(
            screen.char_at(W - 1, H - 2),
            '┘',
            "bottom-right corner missing"
        );
    }

    #[test]
    fn first_frame_draws_side_walls() {
        let state = make_state(W as u16, H as u16);
        let screen = one_frame(&state, true);
        for row in 2..H - 2 {
            assert_eq!(
                screen.char_at(0, row),
                '│',
                "left wall missing at row {row}"
            );
            assert_eq!(
                screen.char_at(W - 1, row),
                '│',
                "right wall missing at row {row}"
            );
        }
    }

    #[test]
    fn first_frame_draws_controls_hint() {
        let state = make_state(W as u16, H as u16);
        let screen = one_frame(&state, true);
        let hint_row = screen.row_str(H - 1);
        assert!(hint_row.contains('Q'), "controls hint missing Q");
        assert!(hint_row.contains("Move"), "controls hint missing Move");
    }

    // ── HUD tests ─────────────────────────────────────────────────────────────

    #[test]
    fn hud_shows_score_label() {
        let mut state = make_state(W as u16, H as u16);
        state.score = 42;
        let screen = one_frame(&state, true);
        assert!(
            screen.row_str(0).contains("Score"),
            "HUD missing Score label"
        );
    }

    #[test]
    fn hud_shows_lives_hearts() {
        let state = make_state(W as u16, H as u16); // 3 lives
        let screen = one_frame(&state, true);
        assert!(screen.row_str(0).contains('♥'), "HUD missing ♥ heart");
    }

    #[test]
    fn hud_shows_hi_score_when_nonzero() {
        // Use a wide screen so the score label and the centred level label don't overlap.
        let mut state = make_state(80, H as u16);
        state.score = 10;
        state.high_score = 100;
        let screen = one_frame(&state, true);
        assert!(screen.row_str(0).contains("Hi"), "HUD missing Hi: label");
    }

    // ── Game-over overlay test ────────────────────────────────────────────────

    #[test]
    fn game_over_overlay_appears() {
        let mut state = make_state(W as u16, H as u16);
        state.status = GameStatus::GameOver;
        let screen = one_frame(&state, true);
        let all: String = (0..H)
            .map(|r| screen.row_str(r))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all.contains("GAME"), "game-over overlay missing GAME");
        assert!(all.contains("OVER"), "game-over overlay missing OVER");
    }

    // ── Ghost / subsequent-frame tests ────────────────────────────────────────

    /// Simulate two consecutive frames with accumulated screen state.
    fn two_frames(state1: &EntireGameStateInfo, state2: &EntireGameStateInfo) -> VirtualScreen {
        let mut screen = VirtualScreen::new(state1.width as usize, state1.height as usize);
        let mut buf = Vec::new();
        render(&mut buf, state1, true).unwrap();
        screen.apply(&buf);
        buf.clear();
        render(&mut buf, state2, false).unwrap();
        screen.apply(&buf);
        screen
    }

    #[test]
    fn subsequent_frame_walls_redrawn() {
        let state = make_state(W as u16, H as u16);
        let screen = two_frames(&state, &state);
        for row in 2..H - 2 {
            assert_eq!(
                screen.char_at(0, row),
                '│',
                "left wall missing at row {row}"
            );
            assert_eq!(
                screen.char_at(W - 1, row),
                '│',
                "right wall missing at row {row}"
            );
        }
    }

    #[test]
    fn no_ghost_at_col0_after_enemy_moves() {
        // Enemy at x=1 renders starting at col 0 (lx = max(0, x-1) = 0).
        // After the enemy disappears, the subsequent-frame clear must erase col 0.
        const ENEMY_ROW: i32 = 5;
        let mut state1 = make_state(W as u16, H as u16);
        state1.enemies = vec![Enemy {
            x: 1,
            y: ENEMY_ROW,
            kind: EnemyKind::Spacecraft,
        }];
        let state2 = make_state(W as u16, H as u16); // no enemies
        let screen = two_frames(&state1, &state2);
        // Col 0 must show the restored wall, not the enemy sprite.
        assert_eq!(
            screen.char_at(0, ENEMY_ROW as usize),
            '│',
            "ghost sprite at col 0 after enemy moved away"
        );
    }

    #[test]
    fn no_ghost_at_right_edge_after_bullet_moves() {
        // A bullet at x = w-1 renders at the right border column.
        // After it moves, col w-1 must be restored to the wall glyph.
        const BULLET_ROW: i32 = 8;
        let mut state1 = make_state(W as u16, H as u16);
        state1.bullets = vec![Bullet {
            x: W as i32 - 1,
            y: BULLET_ROW,
            owner: BulletOwner::Enemy,
        }];
        let state2 = make_state(W as u16, H as u16); // no bullets
        let screen = two_frames(&state1, &state2);
        assert_eq!(
            screen.char_at(W - 1, BULLET_ROW as usize),
            '│',
            "ghost sprite at col w-1 after bullet moved away"
        );
    }
}
