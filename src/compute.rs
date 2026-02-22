/// Pure game-logic functions.
///
/// Every public function takes an immutable reference to the current
/// `EntireGameStateInfo` (and, where needed, an RNG handle) and returns a brand-new
/// `EntireGameStateInfo`.  Side effects are limited to the injected RNG.

use rand::Rng;

use crate::entities::{
    Bullet, BulletOwner, Enemy, EnemyKind, EntireGameStateInfo, GameStatus, Level, Player,
};

// ── Difficulty tables ────────────────────────────────────────────────────────

fn enemy_move_interval(level: &Level) -> u64 {
    match level {
        Level::Easy => 14,
        Level::Medium => 8,
        Level::Hard => 4,
    }
}

fn enemy_spawn_rate(level: &Level) -> u64 {
    match level {
        Level::Easy => 90,
        Level::Medium => 55,
        Level::Hard => 28,
    }
}

/// Score awarded per enemy destroyed.
fn score_for(kind: &EnemyKind) -> u32 {
    match kind {
        EnemyKind::Spacecraft => 100,
        EnemyKind::Octopus => 150,
    }
}

// ── Constructors ─────────────────────────────────────────────────────────────

/// Build the initial game state for a given level and terminal dimensions.
pub fn init_state(level: Level, width: u16, height: u16) -> EntireGameStateInfo {
    EntireGameStateInfo {
        player: Player {
            x: (width / 2) as i32,
            y: (height - 4) as i32, // one row higher to fit the 2-row sprite
            lives: 3,
        },
        enemies: Vec::new(),
        bullets: Vec::new(),
        score: 0,
        level,
        status: GameStatus::Playing,
        frame: 0,
        width,
        height,
    }
}

// ── Input-driven state transitions (pure) ───────────────────────────────────

pub fn move_player_left(state: &EntireGameStateInfo) -> EntireGameStateInfo {
    let new_x = (state.player.x - 2).max(1);
    EntireGameStateInfo {
        player: Player {
            x: new_x,
            ..state.player.clone()
        },
        ..state.clone()
    }
}

pub fn move_player_right(state: &EntireGameStateInfo) -> EntireGameStateInfo {
    let new_x = (state.player.x + 2).min(state.width as i32 - 2);
    EntireGameStateInfo {
        player: Player {
            x: new_x,
            ..state.player.clone()
        },
        ..state.clone()
    }
}

/// Fire a bullet from the player — capped at 3 simultaneous bullets.
pub fn player_shoot(state: &EntireGameStateInfo) -> EntireGameStateInfo {
    let active = state
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .count();
    if active >= 3 {
        return state.clone();
    }
    let new_bullet = Bullet {
        x: state.player.x,
        y: state.player.y - 1,
        owner: BulletOwner::Player,
    };
    let mut bullets = state.bullets.clone();
    bullets.push(new_bullet);
    EntireGameStateInfo {
        bullets,
        ..state.clone()
    }
}

// ── Per-frame tick (nearly pure — RNG is injected) ──────────────────────────

/// Advance the simulation by one frame.  All randomness comes through `rng`
/// so callers control determinism (useful for tests with a seeded RNG).
pub fn tick(state: &EntireGameStateInfo, rng: &mut impl Rng) -> EntireGameStateInfo {
    let frame = state.frame + 1;

    // ── 1. Move bullets ──────────────────────────────────────────────────────
    let bullets: Vec<Bullet> = state
        .bullets
        .iter()
        .filter_map(|b| {
            let new_y = match b.owner {
                BulletOwner::Player => b.y - 1,
                BulletOwner::Enemy => b.y + 1,
            };
            // Discard bullets that leave the play area (rows 2 .. height-3)
            if new_y < 2 || new_y > state.height as i32 - 3 {
                None
            } else {
                Some(Bullet { y: new_y, ..b.clone() })
            }
        })
        .collect();

    // ── 2. Move enemies down on their interval ───────────────────────────────
    let move_interval = enemy_move_interval(&state.level);
    let enemies: Vec<Enemy> = if frame % move_interval == 0 {
        state
            .enemies
            .iter()
            .map(|e| Enemy { y: e.y + 1, ..e.clone() })
            .collect()
    } else {
        state.enemies.clone()
    };

    // ── 3. Spawn a new enemy ─────────────────────────────────────────────────
    let spawn_rate = enemy_spawn_rate(&state.level);
    let mut enemies = enemies;
    if frame % spawn_rate == 0 {
        let x = rng.gen_range(1..(state.width as i32 - 1));
        let kind = if rng.gen_bool(0.6) {
            EnemyKind::Spacecraft
        } else {
            EnemyKind::Octopus
        };
        enemies.push(Enemy { x, y: 2, kind });
    }

    // ── 4. Enemies randomly shoot ────────────────────────────────────────────
    let mut bullets = bullets;
    for enemy in &enemies {
        if rng.gen_ratio(1, 220) {
            bullets.push(Bullet {
                x: enemy.x,
                y: enemy.y + 1,
                owner: BulletOwner::Enemy,
            });
        }
    }

    // ── 5. Collision: player bullets ↔ enemies ───────────────────────────────
    let mut killed_enemies: Vec<usize> = Vec::new();
    let mut used_bullets: Vec<usize> = Vec::new();

    for (bi, bullet) in bullets.iter().enumerate() {
        if bullet.owner != BulletOwner::Player {
            continue;
        }
        for (ei, enemy) in enemies.iter().enumerate() {
            // Hit if bullet lands within the 3-wide, 2-tall enemy bounding box
            if (bullet.x - enemy.x).abs() <= 1
                && (bullet.y == enemy.y || bullet.y == enemy.y + 1)
                && !killed_enemies.contains(&ei)
            {
                killed_enemies.push(ei);
                used_bullets.push(bi);
                break;
            }
        }
    }

    let score_gain: u32 = killed_enemies
        .iter()
        .map(|&i| score_for(&enemies[i].kind))
        .sum();

    let enemies: Vec<Enemy> = enemies
        .iter()
        .enumerate()
        .filter(|(i, _)| !killed_enemies.contains(i))
        .map(|(_, e)| e.clone())
        .collect();

    let bullets: Vec<Bullet> = bullets
        .iter()
        .enumerate()
        .filter(|(i, _)| !used_bullets.contains(i))
        .map(|(_, b)| b.clone())
        .collect();

    // ── 6. Collision: enemy bullets ↔ player ─────────────────────────────────
    let mut player_hit = false;
    let mut used_bullets2: Vec<usize> = Vec::new();

    for (bi, bullet) in bullets.iter().enumerate() {
        if bullet.owner != BulletOwner::Enemy {
            continue;
        }
        if bullet.x == state.player.x && bullet.y == state.player.y {
            player_hit = true;
            used_bullets2.push(bi);
        }
    }

    // Enemy reaching the player's row also counts as a hit
    if enemies.iter().any(|e| e.y >= state.player.y) {
        player_hit = true;
    }

    let bullets: Vec<Bullet> = bullets
        .iter()
        .enumerate()
        .filter(|(i, _)| !used_bullets2.contains(i))
        .map(|(_, b)| b.clone())
        .collect();

    // Remove enemies that have gone past the bottom border
    let enemies: Vec<Enemy> = enemies
        .into_iter()
        .filter(|e| e.y < state.height as i32 - 2)
        .collect();

    // ── 7. Update player & status ─────────────────────────────────────────────
    let new_lives = if player_hit {
        state.player.lives.saturating_sub(1)
    } else {
        state.player.lives
    };

    let status = if new_lives == 0 {
        GameStatus::GameOver
    } else {
        GameStatus::Playing
    };

    let player = Player {
        lives: new_lives,
        ..state.player.clone()
    };

    EntireGameStateInfo {
        player,
        enemies,
        bullets,
        score: state.score + score_gain,
        status,
        frame,
        ..state.clone()
    }
}
