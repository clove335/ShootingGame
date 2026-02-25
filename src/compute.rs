/// Pure game-logic functions.
///
/// Every public function takes an immutable reference to the current
/// `EntireGameStateInfo` (and, where needed, an RNG handle) and returns a brand-new
/// `EntireGameStateInfo`.  Side effects are limited to the injected RNG.

use rand::Rng;

use crate::entities::{
    BonusItem, BonusKind, Bullet, BulletOwner, Enemy, EnemyKind, EntireGameStateInfo,
    Explosion, FlameBullet, FirebombProj, GameStatus, Level, Player,
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

fn score_for(kind: &EnemyKind) -> u32 {
    match kind {
        EnemyKind::Spacecraft => 100,
        EnemyKind::Octopus => 150,
    }
}

// ── Bonus-item constants ──────────────────────────────────────────────────────

const BONUS_SPAWN_INTERVAL: u64 = 150;
const BONUS_MOVE_INTERVAL: u64 = 10;
const POWER_UP_DURATION: u32 = 300;
const MAX_LIVES: u32 = 5;

// ── FlameBurst constants ──────────────────────────────────────────────────────

/// Horizontal velocity for the near pair of flame bullets (±18° from vertical).
/// tan(18°) ≈ 0.3249
const FLAME_VX_NEAR: f32 = 0.3249;

/// Horizontal velocity for the far pair of flame bullets (±54° from vertical).
/// tan(54°) ≈ 1.3764
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

/// Squared radius used for the proximity trigger: detonates when any enemy
/// enters this distance (radius = 2 cells).
const EXPLOSION_TRIGGER_RADIUS_SQ: i32 = 4;

/// Frames the explosion visual stays on screen.
const EXPLOSION_DISPLAY_FRAMES: u32 = 10;

// ── Constructors ─────────────────────────────────────────────────────────────

pub fn init_state(level: Level, width: u16, height: u16, high_score: u32) -> EntireGameStateInfo {
    EntireGameStateInfo {
        player: Player {
            x: (width / 2) as i32,
            y: (height - 4) as i32,
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
    }
}

// ── Input-driven state transitions (pure) ───────────────────────────────────

pub fn move_player_left(state: &EntireGameStateInfo) -> EntireGameStateInfo {
    let new_x = (state.player.x - 2).max(1);
    EntireGameStateInfo {
        player: Player { x: new_x, ..state.player.clone() },
        ..state.clone()
    }
}

pub fn move_player_right(state: &EntireGameStateInfo) -> EntireGameStateInfo {
    let new_x = (state.player.x + 2).min(state.width as i32 - 2);
    EntireGameStateInfo {
        player: Player { x: new_x, ..state.player.clone() },
        ..state.clone()
    }
}

/// Fire a weapon based on the active power-up:
///
/// | Power-up   | What fires                                                  |
/// |------------|-------------------------------------------------------------|
/// | FlameBurst | 4 diagonal `FlameBullet`s at ±18° and ±54° from vertical   |
/// | Firebomb   | 1 slow `FirebombProj` (up to `FIREBOMB_CAP` in flight)      |
/// | SpreadShot | 3 standard bullets spread across −2, 0, +2 columns          |
/// | RapidFire  | 1 standard bullet; cap raised to 6 simultaneous bullets    |
/// | None       | 1 standard bullet; cap of 3 simultaneous bullets           |
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
            EntireGameStateInfo { flame_bullets: flames, ..state.clone() }
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
            EntireGameStateInfo { firebombs: bombs, ..state.clone() }
        }

        // ── Standard bullets (SpreadShot / RapidFire / Normal) ───────────────
        power_up => {
            let is_spread = matches!(power_up, Some((BonusKind::SpreadShot, _)));
            let is_rapid  = matches!(power_up, Some((BonusKind::RapidFire, _)));
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
                for dx in [-2i32, 0, 2] {
                    if bullets
                        .iter()
                        .filter(|b| b.owner == BulletOwner::Player)
                        .count()
                        >= cap
                    {
                        break;
                    }
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
            EntireGameStateInfo { bullets, ..state.clone() }
        }
    }
}

// ── Per-frame tick ────────────────────────────────────────────────────────────

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
                Some(Bullet { y: new_y, ..b.clone() })
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
                Some(FlameBullet { x: nx, y: ny, vx: fb.vx })
            }
        })
        .collect();

    // ── 3. Move enemies ───────────────────────────────────────────────────────
    let move_interval = enemy_move_interval(&state.level);
    let enemies: Vec<Enemy> = if frame % move_interval == 0 {
        state.enemies.iter().map(|e| Enemy { y: e.y + 1, ..e.clone() }).collect()
    } else {
        state.enemies.clone()
    };

    // ── 4. Spawn a new enemy ──────────────────────────────────────────────────
    let mut enemies = enemies;
    let spawn_rate = enemy_spawn_rate(&state.level);
    if frame % spawn_rate == 0 {
        let x = rng.gen_range(1..(w - 1));
        let kind = if rng.gen_bool(0.6) { EnemyKind::Spacecraft } else { EnemyKind::Octopus };
        enemies.push(Enemy { x, y: 2, kind });
    }

    // ── 5. Enemies randomly shoot ─────────────────────────────────────────────
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

    // ── 6. Collision: standard player bullets ↔ enemies ──────────────────────
    let mut killed: Vec<usize> = Vec::new();
    let mut used_bullets: Vec<usize> = Vec::new();
    for (bi, b) in bullets.iter().enumerate() {
        if b.owner != BulletOwner::Player {
            continue;
        }
        for (ei, e) in enemies.iter().enumerate() {
            if (b.x - e.x).abs() <= 1
                && (b.y == e.y || b.y == e.y + 1)
                && !killed.contains(&ei)
            {
                killed.push(ei);
                used_bullets.push(bi);
                break;
            }
        }
    }
    let mut score_gain: u32 = killed.iter().map(|&i| score_for(&enemies[i].kind)).sum();
    let enemies: Vec<Enemy> = enemies
        .iter().enumerate()
        .filter(|(i, _)| !killed.contains(i))
        .map(|(_, e)| e.clone())
        .collect();
    let bullets: Vec<Bullet> = bullets
        .iter().enumerate()
        .filter(|(i, _)| !used_bullets.contains(i))
        .map(|(_, b)| b.clone())
        .collect();

    // ── 7. Collision: flame bullets ↔ enemies ────────────────────────────────
    let mut flame_killed: Vec<usize> = Vec::new();
    let mut used_flames: Vec<usize> = Vec::new();
    for (fi, fb) in flame_bullets.iter().enumerate() {
        let bx = fb.x.round() as i32;
        let by = fb.y.round() as i32;
        for (ei, e) in enemies.iter().enumerate() {
            if (bx - e.x).abs() <= 1
                && (by == e.y || by == e.y + 1)
                && !flame_killed.contains(&ei)
            {
                flame_killed.push(ei);
                used_flames.push(fi);
                break;
            }
        }
    }
    score_gain += flame_killed.iter().map(|&i| score_for(&enemies[i].kind)).sum::<u32>();
    let enemies: Vec<Enemy> = enemies
        .iter().enumerate()
        .filter(|(i, _)| !flame_killed.contains(i))
        .map(|(_, e)| e.clone())
        .collect();
    let flame_bullets: Vec<FlameBullet> = flame_bullets
        .iter().enumerate()
        .filter(|(i, _)| !used_flames.contains(i))
        .map(|(_, fb)| fb.clone())
        .collect();

    // ── 8. Collision: enemy bullets ↔ player ─────────────────────────────────
    let mut player_hit = false;
    let mut used_enemy_bullets: Vec<usize> = Vec::new();
    for (bi, b) in bullets.iter().enumerate() {
        if b.owner != BulletOwner::Enemy {
            continue;
        }
        if b.x == state.player.x && b.y == state.player.y {
            player_hit = true;
            used_enemy_bullets.push(bi);
        }
    }
    if enemies.iter().any(|e| e.y >= state.player.y) {
        player_hit = true;
    }
    let bullets: Vec<Bullet> = bullets
        .iter().enumerate()
        .filter(|(i, _)| !used_enemy_bullets.contains(i))
        .map(|(_, b)| b.clone())
        .collect();

    // Remove enemies that exited the bottom border
    let enemies: Vec<Enemy> = enemies
        .into_iter()
        .filter(|e| e.y < h - 2)
        .collect();

    // ── 9. Move firebombs + detect detonation ────────────────────────────────
    //
    // A firebomb detonates when:
    //   a) it directly contacts an enemy (proximity trigger), or
    //   b) it reaches the top of the play area (y ≤ 2), or
    //   c) its fuse runs out (fuse == 0 after decrement).
    //
    // Detonation kills every enemy within EXPLOSION_KILL_RADIUS_SQ.

    // First, move bombs that are on their movement frame and tick fuses.
    let firebombs_moved: Vec<FirebombProj> = state.firebombs.iter().map(|bomb| {
        let new_y = if frame % FIREBOMB_MOVE_INTERVAL == 0 { bomb.y - 1 } else { bomb.y };
        FirebombProj { y: new_y, fuse: bomb.fuse.saturating_sub(1), ..*bomb }
    }).collect();

    // Partition: bombs that detonate this frame vs. bombs that keep flying.
    let mut detonation_points: Vec<(i32, i32)> = Vec::new();
    let firebombs: Vec<FirebombProj> = firebombs_moved
        .into_iter()
        .filter(|bomb| {
            let proximity_hit = enemies.iter().any(|e| {
                let dx = (e.x - bomb.x) as i32;
                let dy = (e.y - bomb.y) as i32;
                dx * dx + dy * dy <= EXPLOSION_TRIGGER_RADIUS_SQ
            });
            let should_detonate = proximity_hit || bomb.fuse == 0 || bomb.y <= 2;
            if should_detonate {
                detonation_points.push((bomb.x, bomb.y));
            }
            !should_detonate
        })
        .collect();

    // Apply area damage for each detonation.
    let mut bomb_killed: Vec<usize> = Vec::new();
    for &(bx, by) in &detonation_points {
        for (ei, e) in enemies.iter().enumerate() {
            let dx = (e.x - bx) as i32;
            let dy = (e.y - by) as i32;
            if dx * dx + dy * dy <= EXPLOSION_KILL_RADIUS_SQ && !bomb_killed.contains(&ei) {
                bomb_killed.push(ei);
            }
        }
    }
    score_gain += bomb_killed.iter().map(|&i| score_for(&enemies[i].kind)).sum::<u32>();
    let enemies: Vec<Enemy> = enemies
        .iter().enumerate()
        .filter(|(i, _)| !bomb_killed.contains(i))
        .map(|(_, e)| e.clone())
        .collect();

    // ── 10. Tick down existing explosions; add new ones ───────────────────────
    let explosions: Vec<Explosion> = state
        .explosions
        .iter()
        .filter_map(|e| {
            if e.frames > 1 {
                Some(Explosion { frames: e.frames - 1, ..*e })
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

    // ── 11. Move bonus items ──────────────────────────────────────────────────
    let bonus_items: Vec<BonusItem> = if frame % BONUS_MOVE_INTERVAL == 0 {
        state.bonus_items.iter().filter_map(|b| {
            let ny = b.y + 1;
            if ny < h - 2 { Some(BonusItem { y: ny, ..b.clone() }) } else { None }
        }).collect()
    } else {
        state.bonus_items.clone()
    };

    // ── 12. Spawn a new bonus item ────────────────────────────────────────────
    let mut bonus_items = bonus_items;
    if frame % BONUS_SPAWN_INTERVAL == 0 {
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

    // ── 13. Tick down the active power-up ────────────────────────────────────
    let active_power_up = state.active_power_up.as_ref().and_then(|(kind, frames)| {
        if *frames > 1 { Some((kind.clone(), frames - 1)) } else { None }
    });

    // ── 14. Player catches bonus items ────────────────────────────────────────
    let mut extra_lives: u32 = 0;
    let mut new_power_up = active_power_up;
    let bonus_items: Vec<BonusItem> = bonus_items
        .into_iter()
        .filter(|b| {
            let caught = (b.x - state.player.x).abs() <= 1
                && (b.y == state.player.y || b.y == state.player.y + 1);
            if caught {
                match &b.kind {
                    BonusKind::ExtraLife  => { extra_lives += 1; }
                    BonusKind::SpreadShot => { new_power_up = Some((BonusKind::SpreadShot, POWER_UP_DURATION)); }
                    BonusKind::RapidFire  => { new_power_up = Some((BonusKind::RapidFire,  POWER_UP_DURATION)); }
                    BonusKind::FlameBurst => { new_power_up = Some((BonusKind::FlameBurst, POWER_UP_DURATION)); }
                    BonusKind::Firebomb   => { new_power_up = Some((BonusKind::Firebomb,   POWER_UP_DURATION)); }
                }
            }
            !caught
        })
        .collect();

    // ── 15. Update player & status ────────────────────────────────────────────
    let hit_lives = if player_hit { state.player.lives.saturating_sub(1) } else { state.player.lives };
    let new_lives = (hit_lives + extra_lives).min(MAX_LIVES);

    let status = if new_lives == 0 { GameStatus::GameOver } else { GameStatus::Playing };
    let player = Player { lives: new_lives, ..state.player.clone() };
    let new_score = state.score + score_gain;

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
        high_score: state.high_score.max(new_score),
        status,
        frame,
        ..state.clone()
    }
}
