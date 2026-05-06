//! Pure game-logic functions.
//!
//! Every public function takes an immutable reference to the current
//! `EntireGameStateInfo` (and, where needed, an RNG handle) and returns a brand-new
//! `EntireGameStateInfo`.  Side effects are limited to the injected RNG.

use rand::Rng;

use crate::entities::{
    BonusItem, BonusKind, Bullet, BulletOwner, Enemy, EnemyKind, EntireGameStateInfo, Explosion,
    FirebombProj, FlameBullet, GameStatus, Level, Player,
};

// ── Difficulty tables ────────────────────────────────────────────────────────

fn enemy_move_interval(level: &Level) -> u64 {
    match level {
        Level::Easy => 22,   // new — very relaxed
        Level::Medium => 14, // old Easy
        Level::Hard => 8,    // old Medium
        Level::Extreme => 4, // old Hard
    }
}

fn enemy_spawn_rate(level: &Level) -> u64 {
    match level {
        Level::Easy => 130,   // new — fewer opponents
        Level::Medium => 90,  // old Easy
        Level::Hard => 55,    // old Medium
        Level::Extreme => 28, // old Hard
    }
}

/// Score awarded per enemy destroyed.
fn score_for(kind: &EnemyKind) -> u32 {
    match kind {
        EnemyKind::Spacecraft => 100,
        EnemyKind::Octopus => 150,
    }
}

// ── Bonus-item constants ──────────────────────────────────────────────────────

/// Frames between bonus-item drops.
const BONUS_SPAWN_INTERVAL: u64 = 150;
/// Frames between each downward step of a bonus item.
const BONUS_MOVE_INTERVAL: u64 = 10;
/// How many frames a timed power-up lasts (≈10 seconds at 30 FPS).
const POWER_UP_DURATION: u32 = 300;
/// Maximum lives the player can hold.
const MAX_LIVES: u32 = 5;

// ── FlameBurst constants ──────────────────────────────────────────────────────

/// Horizontal velocity for the near pair of flame bullets (±18° from vertical).
const FLAME_VX_NEAR: f32 = 0.3249;
/// Horizontal velocity for the far pair of flame bullets (±54° from vertical).
const FLAME_VX_FAR: f32 = 1.3764;

// ── Firebomb constants ────────────────────────────────────────────────────────

/// The firebomb moves upward every this many frames (slow, heavy projectile).
const FIREBOMB_MOVE_INTERVAL: u64 = 4;
/// Frames until a firebomb auto-detonates even without touching an enemy.
const FIREBOMB_FUSE: u32 = 90;
/// Maximum firebombs in flight at once.
const FIREBOMB_CAP: usize = 2;
/// Squared Euclidean radius for the explosion's kill zone (radius = 4 cells).
const EXPLOSION_KILL_RADIUS_SQ: i32 = 16;
/// Squared radius for the proximity trigger (radius = 2 cells).
const EXPLOSION_TRIGGER_RADIUS_SQ: i32 = 4;
/// Frames the explosion visual stays on screen.
const EXPLOSION_DISPLAY_FRAMES: u32 = 10;
/// Frames the muzzle-flash glyph is visible after firing (≈133 ms at 30 FPS).
const MUZZLE_FLASH_DURATION: u32 = 4;
/// Frames a score-milestone cheer stays on screen (≈3 seconds at 30 FPS).
const CHEER_DURATION: u32 = 90;

/// Score thresholds and their cheer messages (must be ascending).
const SCORE_MILESTONES: &[(u32, &str)] = &[
    (500, "Nice! Hot streak!"),
    (1000, "Great! Keep it up!!"),
    (2000, "Amazing!!"),
    (5000, "Unstoppable!!!"),
    (10000, "LEGENDARY!!!"),
];

// ── Constructors ─────────────────────────────────────────────────────────────

/// Build the initial game state for a given level and terminal dimensions.
pub fn init_state(level: Level, width: u16, height: u16, high_score: u32) -> EntireGameStateInfo {
    EntireGameStateInfo {
        player: Player {
            x: (width / 2) as i32,
            y: (height - 4) as i32, // one row higher to fit the 2-row sprite
            lives: 3,
        },
        enemies: Vec::new(),
        bullets: Vec::new(),
        flame_bullets: Vec::new(),
        firebombs: Vec::new(),
        explosions: Vec::new(),
        bonus_items: Vec::new(),
        active_power_up: None,
        score: 0,
        high_score,
        level,
        status: GameStatus::Playing,
        frame: 0,
        width,
        height,
        debug_mode: false,
        god_mode: false,
        slow_mo: false,
        muzzle_flash: 0,
        cheer_msg: None,
    }
}

// ── Input-driven state transitions (pure) ───────────────────────────────────

pub fn move_player_left(state: &EntireGameStateInfo) -> EntireGameStateInfo {
    move_player_left_n(state, 1)
}

pub fn move_player_right(state: &EntireGameStateInfo) -> EntireGameStateInfo {
    move_player_right_n(state, 1)
}

pub fn move_player_left_n(state: &EntireGameStateInfo, n: i32) -> EntireGameStateInfo {
    let new_x = (state.player.x - n).max(1);
    EntireGameStateInfo {
        player: Player {
            x: new_x,
            ..state.player.clone()
        },
        ..state.clone()
    }
}

pub fn move_player_right_n(state: &EntireGameStateInfo, n: i32) -> EntireGameStateInfo {
    let new_x = (state.player.x + n).min(state.width as i32 - 2);
    EntireGameStateInfo {
        player: Player {
            x: new_x,
            ..state.player.clone()
        },
        ..state.clone()
    }
}

/// Fire a weapon based on the active power-up.
pub fn player_shoot(state: &EntireGameStateInfo) -> EntireGameStateInfo {
    match &state.active_power_up {
        // ── FlameBurst: 4 angled flame bullets ───────────────────────────────
        Some((BonusKind::FlameBurst, _)) => {
            let mut flames = state.flame_bullets.clone();
            for &vx in &[-FLAME_VX_FAR, -FLAME_VX_NEAR, FLAME_VX_NEAR, FLAME_VX_FAR] {
                flames.push(FlameBullet {
                    x: state.player.x as f32,
                    y: (state.player.y - 1) as f32,
                    vx,
                });
            }
            EntireGameStateInfo {
                flame_bullets: flames,
                muzzle_flash: MUZZLE_FLASH_DURATION,
                ..state.clone()
            }
        }

        // ── Firebomb: one slow explosive projectile ───────────────────────────
        Some((BonusKind::Firebomb, _)) => {
            if state.firebombs.len() >= FIREBOMB_CAP {
                return state.clone();
            }
            let mut bombs = state.firebombs.clone();
            bombs.push(FirebombProj {
                x: state.player.x,
                y: state.player.y - 1,
                fuse: FIREBOMB_FUSE,
            });
            EntireGameStateInfo {
                firebombs: bombs,
                muzzle_flash: MUZZLE_FLASH_DURATION,
                ..state.clone()
            }
        }

        // ── Standard bullets (SpreadShot / RapidFire / Normal) ───────────────
        power_up => {
            let is_spread = matches!(power_up, Some((BonusKind::SpreadShot, _)));
            let is_rapid = matches!(power_up, Some((BonusKind::RapidFire, _)));
            let cap = if is_rapid { 6 } else { 3 };

            let active = state
                .bullets
                .iter()
                .filter(|b| b.owner == BulletOwner::Player)
                .count();

            if active >= cap {
                return state.clone();
            }

            let mut bullets = state.bullets.clone();

            if is_spread {
                if active > 0 {
                    return state.clone();
                }
                for &dx in &[-2_i32, 0, 2] {
                    let bx = (state.player.x + dx).clamp(1, state.width as i32 - 2);
                    bullets.push(Bullet {
                        x: bx,
                        y: state.player.y - 1,
                        owner: BulletOwner::Player,
                    });
                }
            } else {
                bullets.push(Bullet {
                    x: state.player.x,
                    y: state.player.y - 1,
                    owner: BulletOwner::Player,
                });
            }

            EntireGameStateInfo {
                bullets,
                muzzle_flash: MUZZLE_FLASH_DURATION,
                ..state.clone()
            }
        }
    }
}

// ── Per-frame tick (nearly pure — RNG is injected) ──────────────────────────

/// Advance the simulation by one frame.  All randomness comes through `rng`
/// so callers control determinism (useful for tests with a seeded RNG).
pub fn tick(state: &EntireGameStateInfo, rng: &mut impl Rng) -> EntireGameStateInfo {
    let frame = state.frame + 1;

    let w = state.width as i32;
    let h = state.height as i32;

    // ── 1. Move standard bullets ─────────────────────────────────────────────
    let bullets: Vec<Bullet> = state
        .bullets
        .iter()
        .filter_map(|b| {
            let new_y = match b.owner {
                BulletOwner::Player => b.y - 1,
                BulletOwner::Enemy => b.y + 1,
            };
            if new_y < 2 || new_y > h - 3 {
                None
            } else {
                Some(Bullet {
                    y: new_y,
                    ..b.clone()
                })
            }
        })
        .collect();

    // ── 2. Move flame bullets (diagonal, float positions) ────────────────────
    let flame_bullets: Vec<FlameBullet> = state
        .flame_bullets
        .iter()
        .filter_map(|fb| {
            let nx = fb.x + fb.vx;
            let ny = fb.y - 1.0;
            if nx < 1.0 || nx > (w - 2) as f32 || ny < 2.0 || ny > (h - 3) as f32 {
                None
            } else {
                Some(FlameBullet {
                    x: nx,
                    y: ny,
                    vx: fb.vx,
                })
            }
        })
        .collect();

    // ── 3. Move enemies down on their interval ───────────────────────────────
    let move_interval = enemy_move_interval(&state.level);
    let enemies: Vec<Enemy> = if frame.is_multiple_of(move_interval) {
        state
            .enemies
            .iter()
            .map(|e| Enemy {
                y: e.y + 1,
                ..e.clone()
            })
            .collect()
    } else {
        state.enemies.clone()
    };

    // ── 3. Spawn a new enemy ─────────────────────────────────────────────────
    let spawn_rate = enemy_spawn_rate(&state.level);
    let mut enemies = enemies;
    if frame.is_multiple_of(spawn_rate) {
        let x = rng.gen_range(2..(state.width as i32 - 2));
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

    let mut score_gain: u32 = killed_enemies
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

    // ── 6. Collision: flame bullets ↔ enemies ────────────────────────────────
    let mut flame_killed: Vec<usize> = Vec::new();
    let mut used_flames: Vec<usize> = Vec::new();
    for (fi, fb) in flame_bullets.iter().enumerate() {
        let bx = fb.x.round() as i32;
        let by = fb.y.round() as i32;
        for (ei, e) in enemies.iter().enumerate() {
            if (bx - e.x).abs() <= 1 && (by == e.y || by == e.y + 1) && !flame_killed.contains(&ei)
            {
                flame_killed.push(ei);
                used_flames.push(fi);
                break;
            }
        }
    }
    score_gain += flame_killed
        .iter()
        .map(|&i| score_for(&enemies[i].kind))
        .sum::<u32>();
    let enemies: Vec<Enemy> = enemies
        .iter()
        .enumerate()
        .filter(|(i, _)| !flame_killed.contains(i))
        .map(|(_, e)| e.clone())
        .collect();
    let flame_bullets: Vec<FlameBullet> = flame_bullets
        .iter()
        .enumerate()
        .filter(|(i, _)| !used_flames.contains(i))
        .map(|(_, fb)| fb.clone())
        .collect();

    // ── 7. Collision: enemy bullets ↔ player ─────────────────────────────────
    let mut player_hit = false;
    let mut used_bullets2: Vec<usize> = Vec::new();

    for (bi, bullet) in bullets.iter().enumerate() {
        if bullet.owner != BulletOwner::Enemy {
            continue;
        }
        if (bullet.x - state.player.x).abs() <= 1
            && (bullet.y == state.player.y || bullet.y == state.player.y + 1)
        {
            player_hit = true;
            used_bullets2.push(bi);
        }
    }

    let bullets: Vec<Bullet> = bullets
        .iter()
        .enumerate()
        .filter(|(i, _)| !used_bullets2.contains(i))
        .map(|(_, b)| b.clone())
        .collect();

    // Enemies that reach the player's row crash into the player (1 life lost)
    // and are removed from the field — no per-frame repeated damage.
    let mut contact_indices: Vec<usize> = Vec::new();
    for (i, e) in enemies.iter().enumerate() {
        if e.y >= state.player.y {
            player_hit = true;
            contact_indices.push(i);
        }
    }
    let enemies: Vec<Enemy> = enemies
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !contact_indices.contains(i))
        .map(|(_, e)| e)
        .collect();

    // Remove enemies that have gone past the bottom border
    let enemies: Vec<Enemy> = enemies.into_iter().filter(|e| e.y < h - 2).collect();

    // ── 8. Move firebombs + detect detonation ────────────────────────────────
    let firebombs_moved: Vec<FirebombProj> = state
        .firebombs
        .iter()
        .map(|bomb| {
            let new_y = if frame.is_multiple_of(FIREBOMB_MOVE_INTERVAL) {
                bomb.y - 1
            } else {
                bomb.y
            };
            FirebombProj {
                y: new_y,
                fuse: bomb.fuse.saturating_sub(1),
                ..*bomb
            }
        })
        .collect();

    let mut detonation_points: Vec<(i32, i32)> = Vec::new();
    let firebombs: Vec<FirebombProj> = firebombs_moved
        .into_iter()
        .filter(|bomb| {
            let proximity_hit = enemies.iter().any(|e| {
                let dx = e.x - bomb.x;
                let dy = e.y - bomb.y;
                dx * dx + dy * dy <= EXPLOSION_TRIGGER_RADIUS_SQ
            });
            let should_detonate = proximity_hit || bomb.fuse == 0 || bomb.y <= 2;
            if should_detonate {
                detonation_points.push((bomb.x, bomb.y));
            }
            !should_detonate
        })
        .collect();

    let mut bomb_killed: Vec<usize> = Vec::new();
    for &(bx, by) in &detonation_points {
        for (ei, e) in enemies.iter().enumerate() {
            let dx = e.x - bx;
            let dy = e.y - by;
            if dx * dx + dy * dy <= EXPLOSION_KILL_RADIUS_SQ && !bomb_killed.contains(&ei) {
                bomb_killed.push(ei);
            }
        }
    }
    score_gain += bomb_killed
        .iter()
        .map(|&i| score_for(&enemies[i].kind))
        .sum::<u32>();
    let enemies: Vec<Enemy> = enemies
        .iter()
        .enumerate()
        .filter(|(i, _)| !bomb_killed.contains(i))
        .map(|(_, e)| e.clone())
        .collect();

    // ── 9. Tick down existing explosions; add new ones ────────────────────────
    let explosions: Vec<Explosion> = state
        .explosions
        .iter()
        .filter_map(|e| {
            if e.frames > 1 {
                Some(Explosion {
                    frames: e.frames - 1,
                    ..*e
                })
            } else {
                None
            }
        })
        .chain(detonation_points.iter().map(|&(x, y)| Explosion {
            x,
            y,
            frames: EXPLOSION_DISPLAY_FRAMES,
        }))
        .collect();

    // ── 10. Move bonus items ──────────────────────────────────────────────────
    let bonus_items: Vec<BonusItem> = if frame.is_multiple_of(BONUS_MOVE_INTERVAL) {
        state
            .bonus_items
            .iter()
            .filter_map(|b| {
                let new_y = b.y + 1;
                if new_y < h - 2 {
                    Some(BonusItem {
                        y: new_y,
                        ..b.clone()
                    })
                } else {
                    None
                }
            })
            .collect()
    } else {
        state.bonus_items.clone()
    };

    // ── 11. Spawn a new bonus item ────────────────────────────────────────────
    let mut bonus_items = bonus_items;
    if frame.is_multiple_of(BONUS_SPAWN_INTERVAL) {
        let x = rng.gen_range(2..(w - 2));
        let kind = match rng.gen_range(0..5u32) {
            0 => BonusKind::SpreadShot,
            1 => BonusKind::ExtraLife,
            2 => BonusKind::RapidFire,
            3 => BonusKind::FlameBurst,
            _ => BonusKind::Firebomb,
        };
        bonus_items.push(BonusItem { x, y: 2, kind });
    }

    // ── 12. Tick down the active power-up ────────────────────────────────────
    let active_power_up = state.active_power_up.as_ref().and_then(|(kind, frames)| {
        if *frames > 1 {
            Some((kind.clone(), frames - 1))
        } else {
            None
        }
    });

    // ── 13. Collision: player catches bonus items ─────────────────────────────
    let mut extra_lives: u32 = 0;
    let mut new_power_up = active_power_up;

    let bonus_items: Vec<BonusItem> = bonus_items
        .into_iter()
        .filter(|b| {
            let caught = (b.x - state.player.x).abs() <= 1
                && (b.y == state.player.y || b.y == state.player.y + 1);
            if caught {
                match &b.kind {
                    BonusKind::ExtraLife => {
                        extra_lives += 1;
                    }
                    BonusKind::SpreadShot => {
                        new_power_up = Some((BonusKind::SpreadShot, POWER_UP_DURATION));
                    }
                    BonusKind::RapidFire => {
                        new_power_up = Some((BonusKind::RapidFire, POWER_UP_DURATION));
                    }
                    BonusKind::FlameBurst => {
                        new_power_up = Some((BonusKind::FlameBurst, POWER_UP_DURATION));
                    }
                    BonusKind::Firebomb => {
                        new_power_up = Some((BonusKind::Firebomb, POWER_UP_DURATION));
                    }
                }
            }
            !caught
        })
        .collect();

    // ── 11. Update player & status ────────────────────────────────────────────
    let hit_lives = if player_hit && !state.god_mode {
        state.player.lives.saturating_sub(1)
    } else {
        state.player.lives
    };
    let new_lives = (hit_lives + extra_lives).min(MAX_LIVES);

    let status = if new_lives == 0 {
        GameStatus::GameOver
    } else {
        GameStatus::Playing
    };

    let player = Player {
        lives: new_lives,
        ..state.player.clone()
    };

    let new_score = state.score + score_gain;
    let new_high_score = state.high_score.max(new_score);

    // ── 12. Tick muzzle flash ─────────────────────────────────────────────────
    let muzzle_flash = state.muzzle_flash.saturating_sub(1);

    // ── 13. Score milestone cheer ─────────────────────────────────────────────
    // A new milestone message always overrides the current one.
    let cheer_msg = SCORE_MILESTONES
        .iter()
        .rev()
        .find(|(threshold, _)| state.score < *threshold && new_score >= *threshold)
        .map(|(_, msg)| (msg.to_string(), CHEER_DURATION))
        .or_else(|| {
            state.cheer_msg.as_ref().and_then(|(msg, frames)| {
                if *frames > 1 {
                    Some((msg.clone(), frames - 1))
                } else {
                    None
                }
            })
        });

    EntireGameStateInfo {
        player,
        enemies,
        bullets,
        flame_bullets,
        firebombs,
        explosions,
        bonus_items,
        active_power_up: new_power_up,
        score: new_score,
        high_score: new_high_score,
        status,
        frame,
        muzzle_flash,
        cheer_msg,
        ..state.clone()
    }
}
