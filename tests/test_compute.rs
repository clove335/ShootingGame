use shooting_game::compute::*;
use shooting_game::entities::*;

use rand::rngs::StdRng;
use rand::SeedableRng;

fn make_state() -> EntireGameStateInfo {
    EntireGameStateInfo {
        player: Player { x: 20, y: 16, lives: 3 },
        enemies: Vec::new(),
        bullets: Vec::new(),
        bonus_items: Vec::new(),
        active_power_up: None,
        score: 0,
        high_score: 0,
        level: Level::Easy,
        status: GameStatus::Playing,
        frame: 0,
        width: 40,
        height: 20,
    }
}

fn seeded_rng() -> StdRng {
    StdRng::seed_from_u64(42)
}

// ── init_state ────────────────────────────────────────────────────────────────

#[test]
fn init_state_player_position() {
    let s = init_state(Level::Easy, 40, 20, 0);
    assert_eq!(s.player.x, 20); // width / 2
    assert_eq!(s.player.y, 16); // height - 4
    assert_eq!(s.player.lives, 3);
}

#[test]
fn init_state_empty_collections() {
    let s = init_state(Level::Easy, 40, 20, 0);
    assert!(s.enemies.is_empty());
    assert!(s.bullets.is_empty());
    assert_eq!(s.score, 0);
    assert_eq!(s.frame, 0);
    assert_eq!(s.status, GameStatus::Playing);
}

#[test]
fn init_state_preserves_level_and_dims() {
    let s = init_state(Level::Hard, 80, 24, 0);
    assert_eq!(s.level, Level::Hard);
    assert_eq!(s.width, 80);
    assert_eq!(s.height, 24);
}

// ── move_player_left ──────────────────────────────────────────────────────────

#[test]
fn move_left_normal() {
    let s = make_state(); // x=20
    let s2 = move_player_left(&s);
    assert_eq!(s2.player.x, 18); // step is 2
}

#[test]
fn move_left_clamps_at_boundary() {
    let mut s = make_state();
    s.player.x = 1;
    let s2 = move_player_left(&s);
    assert_eq!(s2.player.x, 1);
}

#[test]
fn move_left_clamps_near_boundary() {
    let mut s = make_state();
    s.player.x = 2;
    let s2 = move_player_left(&s);
    assert_eq!(s2.player.x, 1); // clamped, not 0
}

// ── move_player_right ─────────────────────────────────────────────────────────

#[test]
fn move_right_normal() {
    let s = make_state(); // x=20
    let s2 = move_player_right(&s);
    assert_eq!(s2.player.x, 22); // step is 2
}

#[test]
fn move_right_clamps_at_boundary() {
    let mut s = make_state();
    s.player.x = 38; // width-2
    let s2 = move_player_right(&s);
    assert_eq!(s2.player.x, 38);
}

#[test]
fn move_right_clamps_near_boundary() {
    let mut s = make_state();
    s.player.x = 37;
    let s2 = move_player_right(&s);
    assert_eq!(s2.player.x, 38); // not 39
}

#[test]
fn move_does_not_mutate_original() {
    let s = make_state();
    let _s2 = move_player_left(&s);
    let _s3 = move_player_right(&s);
    assert_eq!(s.player.x, 20);
}

#[test]
fn movement_backward_compat_left() {
    // ← and A both route to move_player_left — results must be identical
    let s = make_state();
    let via_arrow = move_player_left(&s);
    let via_a_key = move_player_left(&s);
    assert_eq!(via_arrow.player.x, via_a_key.player.x);
}

#[test]
fn movement_backward_compat_right() {
    let s = make_state();
    let via_arrow = move_player_right(&s);
    let via_d_key = move_player_right(&s);
    assert_eq!(via_arrow.player.x, via_d_key.player.x);
}

// ── player_shoot ──────────────────────────────────────────────────────────────

#[test]
fn shoot_adds_bullet_at_player_position() {
    let s = make_state();
    let s2 = player_shoot(&s);
    assert_eq!(s2.bullets.len(), 1);
    let b = &s2.bullets[0];
    assert_eq!(b.x, s.player.x);
    assert_eq!(b.y, s.player.y - 1);
    assert_eq!(b.owner, BulletOwner::Player);
}

#[test]
fn shoot_cap_at_three_player_bullets() {
    let mut s = make_state();
    for _ in 0..3 {
        s.bullets.push(Bullet { x: 5, y: 5, owner: BulletOwner::Player });
    }
    let s2 = player_shoot(&s);
    assert_eq!(s2.bullets.len(), 3); // 4th shot blocked
}

#[test]
fn shoot_respects_cap_with_mixed_bullets() {
    let mut s = make_state();
    // 2 player bullets + 5 enemy bullets — player cap still has room
    for _ in 0..2 {
        s.bullets.push(Bullet { x: 5, y: 5, owner: BulletOwner::Player });
    }
    for _ in 0..5 {
        s.bullets.push(Bullet { x: 3, y: 8, owner: BulletOwner::Enemy });
    }
    let s2 = player_shoot(&s);
    // Should now have 3 player + 5 enemy = 8 total
    let player_count = s2.bullets.iter().filter(|b| b.owner == BulletOwner::Player).count();
    assert_eq!(player_count, 3);
}

#[test]
fn shoot_allows_third_bullet() {
    let mut s = make_state();
    for _ in 0..2 {
        s.bullets.push(Bullet { x: 5, y: 5, owner: BulletOwner::Player });
    }
    let s2 = player_shoot(&s);
    let player_count = s2.bullets.iter().filter(|b| b.owner == BulletOwner::Player).count();
    assert_eq!(player_count, 3);
}

#[test]
fn shoot_does_not_mutate_original() {
    let s = make_state();
    let _ = player_shoot(&s);
    assert!(s.bullets.is_empty());
}

// ── tick — frame counter & bullets ───────────────────────────────────────────

#[test]
fn tick_increments_frame() {
    let mut s = make_state();
    s.frame = 5;
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.frame, 6);
}

#[test]
fn tick_player_bullet_moves_up() {
    let mut s = make_state();
    s.bullets.push(Bullet { x: 20, y: 10, owner: BulletOwner::Player });
    let s2 = tick(&s, &mut seeded_rng());
    // bullet at y=10 → y=9 (and not discarded since 9 >= 2)
    let b: Vec<_> = s2.bullets.iter().filter(|b| b.owner == BulletOwner::Player).collect();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].y, 9);
}

#[test]
fn tick_enemy_bullet_moves_down() {
    let mut s = make_state();
    s.bullets.push(Bullet { x: 20, y: 10, owner: BulletOwner::Enemy });
    let s2 = tick(&s, &mut seeded_rng());
    let b: Vec<_> = s2.bullets.iter().filter(|b| b.owner == BulletOwner::Enemy).collect();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].y, 11);
}

#[test]
fn tick_bullet_discarded_at_top_boundary() {
    let mut s = make_state();
    // y=3 → new_y=2 → kept; y=2 → new_y=1 → discarded
    s.bullets.push(Bullet { x: 20, y: 3, owner: BulletOwner::Player });
    s.bullets.push(Bullet { x: 15, y: 2, owner: BulletOwner::Player });
    let s2 = tick(&s, &mut seeded_rng());
    let kept: Vec<_> = s2.bullets.iter().filter(|b| b.owner == BulletOwner::Player).collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].y, 2);
}

#[test]
fn tick_bullet_discarded_at_bottom_boundary() {
    // height=20, boundary = height-3 = 17; new_y > 17 is discarded
    let mut s = make_state();
    // y=17 → new_y=18 → discarded; y=16 → new_y=17 → kept
    s.bullets.push(Bullet { x: 20, y: 17, owner: BulletOwner::Enemy });
    s.bullets.push(Bullet { x: 15, y: 16, owner: BulletOwner::Enemy });
    let s2 = tick(&s, &mut seeded_rng());
    let kept: Vec<_> = s2.bullets.iter().filter(|b| b.owner == BulletOwner::Enemy).collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].y, 17);
}

// ── tick — enemy movement ─────────────────────────────────────────────────────

#[test]
fn tick_enemies_move_on_interval_easy() {
    // Easy interval = 14; frame 0→1: frame 1 % 14 ≠ 0, no move
    // We want frame N such that N+1 ≡ 0 (mod 14) → N=13
    let mut s = make_state();
    s.frame = 13;
    s.enemies.push(Enemy { x: 10, y: 5, kind: EnemyKind::Spacecraft });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.enemies[0].y, 6); // moved on frame 14
}

#[test]
fn tick_enemies_do_not_move_off_interval() {
    let mut s = make_state();
    s.frame = 1; // next frame = 2, not divisible by 14
    s.enemies.push(Enemy { x: 10, y: 5, kind: EnemyKind::Spacecraft });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.enemies[0].y, 5);
}

#[test]
fn tick_enemy_purged_past_bottom() {
    // height=20, purge when e.y >= height-2 = 18
    // Enemy at y=17 moves to 18 on frame 14 → purged
    let mut s = make_state();
    s.frame = 13;
    s.enemies.push(Enemy { x: 10, y: 17, kind: EnemyKind::Spacecraft });
    let s2 = tick(&s, &mut seeded_rng());
    assert!(s2.enemies.is_empty());
}

// ── tick — collision: player bullet ↔ enemy ──────────────────────────────────

#[test]
fn tick_player_bullet_hits_enemy_direct() {
    // tick() moves bullets BEFORE collision detection.
    // Player bullet moves UP (y-1), so place it one row below the enemy.
    let mut s = make_state();
    s.frame = 1; // frame 2, no movement
    s.enemies.push(Enemy { x: 10, y: 5, kind: EnemyKind::Spacecraft });
    s.bullets.push(Bullet { x: 10, y: 6, owner: BulletOwner::Player }); // moves to y=5
    let s2 = tick(&s, &mut seeded_rng());
    assert!(s2.enemies.is_empty());
    assert_eq!(s2.score, 100);
}

#[test]
fn tick_player_bullet_hits_enemy_wide_box() {
    // Bounding box is 3-wide: x±1 also hits
    let mut s = make_state();
    s.frame = 1;
    s.enemies.push(Enemy { x: 10, y: 5, kind: EnemyKind::Spacecraft });
    s.bullets.push(Bullet { x: 11, y: 6, owner: BulletOwner::Player }); // x+1, moves to y=5
    let s2 = tick(&s, &mut seeded_rng());
    assert!(s2.enemies.is_empty());
}

#[test]
fn tick_player_bullet_misses_enemy_outside_box() {
    let mut s = make_state();
    s.frame = 1;
    s.enemies.push(Enemy { x: 10, y: 5, kind: EnemyKind::Spacecraft });
    s.bullets.push(Bullet { x: 12, y: 5, owner: BulletOwner::Player }); // x+2, outside
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.enemies.len(), 1);
}

#[test]
fn tick_player_bullet_hits_enemy_second_row() {
    // Bounding box is 2-tall: enemy.y+1 also hits
    let mut s = make_state();
    s.frame = 1;
    s.enemies.push(Enemy { x: 10, y: 5, kind: EnemyKind::Spacecraft });
    s.bullets.push(Bullet { x: 10, y: 6, owner: BulletOwner::Player }); // y+1
    let s2 = tick(&s, &mut seeded_rng());
    assert!(s2.enemies.is_empty());
}

#[test]
fn tick_octopus_scores_150() {
    let mut s = make_state();
    s.frame = 1;
    s.enemies.push(Enemy { x: 10, y: 5, kind: EnemyKind::Octopus });
    s.bullets.push(Bullet { x: 10, y: 6, owner: BulletOwner::Player }); // moves to y=5
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.score, 150);
}

// ── tick — collision: enemy bullet ↔ player ──────────────────────────────────

#[test]
fn tick_enemy_bullet_hits_player() {
    // Enemy bullet moves DOWN (y+1) before collision detection.
    // Collision checks against state.player.y (old position = 16).
    // Place bullet at player.y-1 so it moves into player.y.
    let mut s = make_state(); // player at (20, 16)
    s.frame = 1;
    s.bullets.push(Bullet { x: 20, y: 15, owner: BulletOwner::Enemy }); // moves to y=16
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2);
}

#[test]
fn tick_player_loses_life_on_enemy_contact() {
    let mut s = make_state(); // player.y = 16
    s.frame = 1;
    s.enemies.push(Enemy { x: 5, y: 16, kind: EnemyKind::Spacecraft });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2);
}

#[test]
fn tick_game_over_when_lives_reach_zero() {
    let mut s = make_state();
    s.player.lives = 1;
    s.frame = 1;
    s.bullets.push(Bullet { x: 20, y: 15, owner: BulletOwner::Enemy }); // moves to y=16
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 0);
    assert_eq!(s2.status, GameStatus::GameOver);
}

#[test]
fn tick_no_game_over_when_lives_above_zero() {
    let mut s = make_state();
    s.player.lives = 2;
    s.frame = 1;
    s.bullets.push(Bullet { x: 20, y: 15, owner: BulletOwner::Enemy }); // moves to y=16
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 1);
    assert_eq!(s2.status, GameStatus::Playing);
}

#[test]
fn tick_lives_saturate_at_zero() {
    let mut s = make_state();
    s.player.lives = 0;
    s.frame = 1;
    s.bullets.push(Bullet { x: 20, y: 16, owner: BulletOwner::Enemy });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 0); // saturating_sub, no underflow
}

// ── tick — enemy spawn ────────────────────────────────────────────────────────

#[test]
fn tick_enemy_spawns_on_interval() {
    // Easy spawn_rate = 90; frame 89 → next frame 90 → spawn
    let mut s = make_state();
    s.frame = 89;
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.enemies.len(), 1);
    assert_eq!(s2.enemies[0].y, 2);
}

#[test]
fn tick_no_spawn_off_interval() {
    let mut s = make_state();
    s.frame = 1; // next frame = 2, not 90
    let s2 = tick(&s, &mut seeded_rng());
    assert!(s2.enemies.is_empty());
}
