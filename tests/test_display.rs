use shooting_game::display::render;
use shooting_game::entities::{
    Bullet, BulletOwner, Enemy, EnemyKind, EntireGameStateInfo, GameStatus, Level, Player,
};

// ── Virtual terminal emulator ─────────────────────────────────────────────────
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

// ── Helpers ───────────────────────────────────────────────────────────────────

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

// ── Border tests ──────────────────────────────────────────────────────────────

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

// ── HUD tests ─────────────────────────────────────────────────────────────────

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

// ── Game-over overlay test ────────────────────────────────────────────────────

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

// ── Ghost / subsequent-frame tests ────────────────────────────────────────────

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
