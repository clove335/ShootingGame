use shooting_game::compute::*;
use shooting_game::entities::*;

use rand::rngs::StdRng;
use rand::SeedableRng;

fn make_state() -> EntireGameStateInfo {
    EntireGameStateInfo {
        player: Player {
            x: 20,
            y: 16,
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
        high_score: 0,
        level: Level::Easy,
        status: GameStatus::Playing,
        frame: 0,
        width: 40,
        height: 20,
        debug_mode: false,
        god_mode: false,
        slow_mo: false,
        muzzle_flash: 0,
        cheer_msg: None,
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
    assert_eq!(s2.player.x, 19); // step is 1
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
    assert_eq!(s2.player.x, 21); // step is 1
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
        s.bullets.push(Bullet {
            x: 5,
            y: 5,
            owner: BulletOwner::Player,
        });
    }
    let s2 = player_shoot(&s);
    assert_eq!(s2.bullets.len(), 3); // 4th shot blocked
}

#[test]
fn shoot_respects_cap_with_mixed_bullets() {
    let mut s = make_state();
    // 2 player bullets + 5 enemy bullets — player cap still has room
    for _ in 0..2 {
        s.bullets.push(Bullet {
            x: 5,
            y: 5,
            owner: BulletOwner::Player,
        });
    }
    for _ in 0..5 {
        s.bullets.push(Bullet {
            x: 3,
            y: 8,
            owner: BulletOwner::Enemy,
        });
    }
    let s2 = player_shoot(&s);
    // Should now have 3 player + 5 enemy = 8 total
    let player_count = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .count();
    assert_eq!(player_count, 3);
}

#[test]
fn shoot_allows_third_bullet() {
    let mut s = make_state();
    for _ in 0..2 {
        s.bullets.push(Bullet {
            x: 5,
            y: 5,
            owner: BulletOwner::Player,
        });
    }
    let s2 = player_shoot(&s);
    let player_count = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .count();
    assert_eq!(player_count, 3);
}

#[test]
fn shoot_does_not_mutate_original() {
    let s = make_state();
    let _ = player_shoot(&s);
    assert!(s.bullets.is_empty());
}

// ── shoot — muzzle flash ──────────────────────────────────────────────────────

#[test]
fn shoot_sets_muzzle_flash() {
    let s = make_state();
    let s2 = player_shoot(&s);
    assert!(s2.muzzle_flash > 0, "muzzle_flash must be set after firing");
}

#[test]
fn shoot_no_muzzle_flash_when_capped() {
    // When the bullet cap is already full, player_shoot is a no-op — no flash.
    let mut s = make_state();
    for _ in 0..3 {
        s.bullets.push(Bullet {
            x: 5,
            y: 5,
            owner: BulletOwner::Player,
        });
    }
    let s2 = player_shoot(&s);
    assert_eq!(
        s2.muzzle_flash, 0,
        "muzzle_flash must stay 0 when shot is blocked"
    );
}

// ── shoot — SpreadShot ────────────────────────────────────────────────────────

#[test]
fn spreadshot_fires_three_bullets() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::SpreadShot, 300));
    let s2 = player_shoot(&s);
    let pb: Vec<_> = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .collect();
    assert_eq!(pb.len(), 3);
}

#[test]
fn spreadshot_bullet_columns_spread() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::SpreadShot, 300));
    let s2 = player_shoot(&s);
    let mut xs: Vec<i32> = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .map(|b| b.x)
        .collect();
    xs.sort();
    // Centre shot at player.x, side shots at player.x ± 2
    assert_eq!(xs, vec![s.player.x - 2, s.player.x, s.player.x + 2]);
}

#[test]
fn spreadshot_all_bullets_one_row_above_player() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::SpreadShot, 300));
    let s2 = player_shoot(&s);
    for b in s2.bullets.iter().filter(|b| b.owner == BulletOwner::Player) {
        assert_eq!(b.y, s.player.y - 1);
    }
}

#[test]
fn spreadshot_blocked_while_any_player_bullet_on_screen() {
    // SpreadShot fires a burst of 3 only when no player bullet is already live.
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::SpreadShot, 300));
    s.bullets.push(Bullet {
        x: 5,
        y: 5,
        owner: BulletOwner::Player,
    });
    let s2 = player_shoot(&s);
    let pb_count = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .count();
    assert_eq!(
        pb_count, 1,
        "SpreadShot must not fire while a bullet is live"
    );
}

// ── shoot — RapidFire ─────────────────────────────────────────────────────────

#[test]
fn rapidfire_cap_is_six() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::RapidFire, 300));
    // Pre-load 5 player bullets — one more should be allowed.
    for i in 0..5 {
        s.bullets.push(Bullet {
            x: 20,
            y: 5 + i,
            owner: BulletOwner::Player,
        });
    }
    let s2 = player_shoot(&s);
    let pb_count = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .count();
    assert_eq!(pb_count, 6);
}

#[test]
fn rapidfire_blocked_at_six() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::RapidFire, 300));
    for i in 0..6 {
        s.bullets.push(Bullet {
            x: 20,
            y: 3 + i,
            owner: BulletOwner::Player,
        });
    }
    let s2 = player_shoot(&s);
    let pb_count = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .count();
    assert_eq!(pb_count, 6, "7th shot must be blocked under RapidFire");
}

// ── shoot — FlameBurst ────────────────────────────────────────────────────────

#[test]
fn flameburst_fires_four_flame_bullets() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::FlameBurst, 300));
    let s2 = player_shoot(&s);
    assert_eq!(s2.flame_bullets.len(), 4);
}

#[test]
fn flameburst_does_not_add_standard_bullets() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::FlameBurst, 300));
    let s2 = player_shoot(&s);
    assert!(s2.bullets.is_empty());
}

#[test]
fn flameburst_bullets_spawn_at_player_tip() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::FlameBurst, 300));
    let s2 = player_shoot(&s);
    for fb in &s2.flame_bullets {
        assert_eq!(fb.x, s.player.x as f32);
        assert_eq!(fb.y, (s.player.y - 1) as f32);
    }
}

#[test]
fn flameburst_has_two_near_and_two_far_velocities() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::FlameBurst, 300));
    let s2 = player_shoot(&s);
    let mut vxs: Vec<f32> = s2.flame_bullets.iter().map(|fb| fb.vx).collect();
    vxs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Two symmetric pairs: -FAR, -NEAR, +NEAR, +FAR
    assert!(vxs[0] < 0.0 && vxs[1] < 0.0, "two leftward velocities");
    assert!(vxs[2] > 0.0 && vxs[3] > 0.0, "two rightward velocities");
    assert!(
        (vxs[0].abs() - vxs[3].abs()) < 0.001,
        "FAR velocities symmetric"
    );
    assert!(
        (vxs[1].abs() - vxs[2].abs()) < 0.001,
        "NEAR velocities symmetric"
    );
    assert!(vxs[1].abs() < vxs[0].abs(), "NEAR angle narrower than FAR");
}

#[test]
fn flameburst_accumulates_across_shots() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::FlameBurst, 300));
    let s2 = player_shoot(&s);
    let s3 = player_shoot(&s2);
    assert_eq!(
        s3.flame_bullets.len(),
        8,
        "second shot adds 4 more flame bullets"
    );
}

// ── shoot — Firebomb ──────────────────────────────────────────────────────────

#[test]
fn firebomb_fires_one_proj() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::Firebomb, 300));
    let s2 = player_shoot(&s);
    assert_eq!(s2.firebombs.len(), 1);
}

#[test]
fn firebomb_does_not_add_standard_bullets() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::Firebomb, 300));
    let s2 = player_shoot(&s);
    assert!(s2.bullets.is_empty());
}

#[test]
fn firebomb_spawns_at_player_tip() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::Firebomb, 300));
    let s2 = player_shoot(&s);
    let bomb = &s2.firebombs[0];
    assert_eq!(bomb.x, s.player.x);
    assert_eq!(bomb.y, s.player.y - 1);
}

#[test]
fn firebomb_fuse_is_nonzero() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::Firebomb, 300));
    let s2 = player_shoot(&s);
    assert!(s2.firebombs[0].fuse > 0);
}

#[test]
fn firebomb_cap_at_two() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::Firebomb, 300));
    // Pre-load 2 firebombs — third shot must be blocked.
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 80,
    });
    s.firebombs.push(FirebombProj {
        x: 22,
        y: 8,
        fuse: 70,
    });
    let s2 = player_shoot(&s);
    assert_eq!(s2.firebombs.len(), 2, "firebomb cap must be 2");
}

#[test]
fn firebomb_no_muzzle_flash_when_capped() {
    let mut s = make_state();
    s.active_power_up = Some((BonusKind::Firebomb, 300));
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 80,
    });
    s.firebombs.push(FirebombProj {
        x: 22,
        y: 8,
        fuse: 70,
    });
    let s2 = player_shoot(&s);
    assert_eq!(s2.muzzle_flash, 0, "no flash when firebomb shot is blocked");
}

// ── tick — flame bullet movement ─────────────────────────────────────────────

#[test]
fn tick_flame_bullet_moves_up_and_diagonally() {
    let mut s = make_state();
    s.flame_bullets.push(FlameBullet {
        x: 20.0,
        y: 10.0,
        vx: 0.5,
    });
    let s2 = tick(&s, &mut seeded_rng());
    let fb = s2.flame_bullets.iter().find(|fb| (fb.y - 9.0).abs() < 0.01);
    assert!(fb.is_some(), "flame bullet must move up one row per tick");
    let fb = fb.unwrap();
    assert!(
        (fb.x - 20.5).abs() < 0.01,
        "flame bullet x must shift by vx"
    );
}

#[test]
fn tick_flame_bullet_discarded_at_top_boundary() {
    let mut s = make_state();
    // y=3 -> 2 survives one tick, then y=2 -> 1 is discarded next tick.
    s.flame_bullets.push(FlameBullet {
        x: 20.0,
        y: 3.0,
        vx: 0.0,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.flame_bullets.len(), 1);
    assert!((s2.flame_bullets[0].y - 2.0).abs() < 0.01);
    let s3 = tick(&s2, &mut seeded_rng());
    assert!(
        s3.flame_bullets.is_empty(),
        "flame bullet must be discarded after crossing above y=2 boundary"
    );
}

#[test]
fn tick_flame_bullet_discarded_out_of_bounds_left() {
    let mut s = make_state();
    // Bullet moving hard left, starting near the left wall.
    s.flame_bullets.push(FlameBullet {
        x: 1.5,
        y: 10.0,
        vx: -2.0,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert!(
        s2.flame_bullets.is_empty(),
        "flame bullet moving off left edge must be discarded"
    );
}

// ── tick — flame bullet collision ─────────────────────────────────────────────

#[test]
fn tick_flame_bullet_kills_enemy_on_hit() {
    let mut s = make_state();
    s.enemies.push(Enemy {
        x: 20,
        y: 8,
        kind: EnemyKind::Spacecraft,
    });
    // Place bullet so that after moving up (y 9→8) it lands on the enemy.
    s.flame_bullets.push(FlameBullet {
        x: 20.0,
        y: 9.0,
        vx: 0.0,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert!(
        s2.enemies.is_empty(),
        "flame bullet must kill enemy it hits"
    );
    assert!(
        s2.flame_bullets.is_empty(),
        "used flame bullet must be consumed"
    );
}

#[test]
fn tick_flame_bullet_scores_on_kill() {
    let mut s = make_state();
    s.enemies.push(Enemy {
        x: 20,
        y: 8,
        kind: EnemyKind::Spacecraft,
    });
    s.flame_bullets.push(FlameBullet {
        x: 20.0,
        y: 9.0,
        vx: 0.0,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(
        s2.score, 100,
        "flame kill must award points like a standard bullet"
    );
}

// ── tick — firebomb movement ──────────────────────────────────────────────────

#[test]
fn tick_firebomb_fuse_decrements_every_frame() {
    let mut s = make_state();
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 90,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.firebombs[0].fuse, 89);
}

#[test]
fn tick_firebomb_moves_up_on_interval() {
    // FIREBOMB_MOVE_INTERVAL = 4: a bomb at y=10 must move to y=9 on frame 4.
    let mut s = make_state();
    s.frame = 3; // next tick is frame 4
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 90,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(
        s2.firebombs[0].y, 9,
        "firebomb must move up on FIREBOMB_MOVE_INTERVAL"
    );
}

#[test]
fn tick_firebomb_does_not_move_off_interval() {
    let mut s = make_state();
    s.frame = 4; // next tick is frame 5 — not a multiple of 4
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 90,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(
        s2.firebombs[0].y, 10,
        "firebomb must stay put between move intervals"
    );
}

// ── tick — firebomb detonation ────────────────────────────────────────────────

#[test]
fn tick_firebomb_detonates_on_fuse_expiry() {
    let mut s = make_state();
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 1,
    });
    let s2 = tick(&s, &mut seeded_rng());
    // fuse ticks to 0 → bomb must be removed (detonated)
    assert!(
        s2.firebombs.is_empty(),
        "firebomb must detonate when fuse hits 0"
    );
}

#[test]
fn tick_firebomb_detonation_spawns_explosion() {
    let mut s = make_state();
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 1,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert!(
        !s2.explosions.is_empty(),
        "detonation must spawn an Explosion"
    );
}

#[test]
fn tick_firebomb_kills_enemy_in_blast_radius() {
    let mut s = make_state();
    // Enemy 3 cells away from bomb — within kill radius (r=4, r²=16; dx=3,dy=0 → 9 ≤ 16).
    s.enemies.push(Enemy {
        x: 23,
        y: 10,
        kind: EnemyKind::Spacecraft,
    });
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 1,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert!(
        s2.enemies.is_empty(),
        "enemy in blast radius must be killed"
    );
}

#[test]
fn tick_firebomb_does_not_kill_enemy_outside_blast_radius() {
    let mut s = make_state();
    // Enemy 5 cells away — dx=5, dy=0 → 25 > 16; outside kill radius.
    s.enemies.push(Enemy {
        x: 25,
        y: 10,
        kind: EnemyKind::Spacecraft,
    });
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 1,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(
        s2.enemies.len(),
        1,
        "enemy outside blast radius must survive"
    );
}

#[test]
fn tick_firebomb_scores_for_blast_kill() {
    let mut s = make_state();
    s.enemies.push(Enemy {
        x: 23,
        y: 10,
        kind: EnemyKind::Spacecraft,
    });
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 1,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.score, 100, "blast kill must award points");
}

#[test]
fn tick_firebomb_proximity_detonation() {
    // Bomb within EXPLOSION_TRIGGER_RADIUS_SQ=4 (r=2) of an enemy must auto-detonate.
    let mut s = make_state();
    s.enemies.push(Enemy {
        x: 21,
        y: 10,
        kind: EnemyKind::Octopus,
    }); // dx=1, dy=0 → dist²=1 ≤ 4
    s.firebombs.push(FirebombProj {
        x: 20,
        y: 10,
        fuse: 90,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert!(
        s2.firebombs.is_empty(),
        "proximity to enemy must trigger detonation"
    );
}

// ── tick — explosion countdown ────────────────────────────────────────────────

#[test]
fn tick_explosion_frames_decrements() {
    let mut s = make_state();
    s.explosions.push(Explosion {
        x: 20,
        y: 10,
        frames: 5,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.explosions[0].frames, 4);
}

#[test]
fn tick_explosion_removed_when_frames_reach_zero() {
    let mut s = make_state();
    s.explosions.push(Explosion {
        x: 20,
        y: 10,
        frames: 1,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert!(
        s2.explosions.is_empty(),
        "expired explosion must be removed"
    );
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
    s.bullets.push(Bullet {
        x: 20,
        y: 10,
        owner: BulletOwner::Player,
    });
    let s2 = tick(&s, &mut seeded_rng());
    // bullet at y=10 → y=9 (and not discarded since 9 >= 2)
    let b: Vec<_> = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .collect();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].y, 9);
}

#[test]
fn tick_enemy_bullet_moves_down() {
    let mut s = make_state();
    s.bullets.push(Bullet {
        x: 20,
        y: 10,
        owner: BulletOwner::Enemy,
    });
    let s2 = tick(&s, &mut seeded_rng());
    let b: Vec<_> = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Enemy)
        .collect();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].y, 11);
}

#[test]
fn tick_bullet_discarded_at_top_boundary() {
    let mut s = make_state();
    // y=3 → new_y=2 → kept; y=2 → new_y=1 → discarded
    s.bullets.push(Bullet {
        x: 20,
        y: 3,
        owner: BulletOwner::Player,
    });
    s.bullets.push(Bullet {
        x: 15,
        y: 2,
        owner: BulletOwner::Player,
    });
    let s2 = tick(&s, &mut seeded_rng());
    let kept: Vec<_> = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Player)
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].y, 2);
}

#[test]
fn tick_bullet_discarded_at_bottom_boundary() {
    // height=20, boundary = height-3 = 17; new_y > 17 is discarded
    let mut s = make_state();
    // y=17 → new_y=18 → discarded; y=16 → new_y=17 → kept
    s.bullets.push(Bullet {
        x: 20,
        y: 17,
        owner: BulletOwner::Enemy,
    });
    s.bullets.push(Bullet {
        x: 15,
        y: 16,
        owner: BulletOwner::Enemy,
    });
    let s2 = tick(&s, &mut seeded_rng());
    let kept: Vec<_> = s2
        .bullets
        .iter()
        .filter(|b| b.owner == BulletOwner::Enemy)
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].y, 17);
}

// ── tick — enemy movement ─────────────────────────────────────────────────────

#[test]
fn tick_enemies_move_on_interval_easy() {
    // Easy interval = 22; we want frame N such that N+1 ≡ 0 (mod 22) → N=21
    let mut s = make_state();
    s.frame = 21;
    s.enemies.push(Enemy {
        x: 10,
        y: 5,
        kind: EnemyKind::Spacecraft,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.enemies[0].y, 6); // moved on frame 22
}

#[test]
fn tick_enemies_do_not_move_off_interval() {
    let mut s = make_state();
    s.frame = 1; // next frame = 2, not divisible by 14
    s.enemies.push(Enemy {
        x: 10,
        y: 5,
        kind: EnemyKind::Spacecraft,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.enemies[0].y, 5);
}

#[test]
fn tick_enemy_purged_past_bottom() {
    // height=20, purge when e.y >= height-2 = 18
    // Enemy at y=17 moves to 18 on frame 22 → purged
    let mut s = make_state();
    s.frame = 21;
    s.enemies.push(Enemy {
        x: 10,
        y: 17,
        kind: EnemyKind::Spacecraft,
    });
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
    s.enemies.push(Enemy {
        x: 10,
        y: 5,
        kind: EnemyKind::Spacecraft,
    });
    s.bullets.push(Bullet {
        x: 10,
        y: 6,
        owner: BulletOwner::Player,
    }); // moves to y=5
    let s2 = tick(&s, &mut seeded_rng());
    assert!(s2.enemies.is_empty());
    assert_eq!(s2.score, 100);
}

#[test]
fn tick_player_bullet_hits_enemy_wide_box() {
    // Bounding box is 3-wide: x±1 also hits
    let mut s = make_state();
    s.frame = 1;
    s.enemies.push(Enemy {
        x: 10,
        y: 5,
        kind: EnemyKind::Spacecraft,
    });
    s.bullets.push(Bullet {
        x: 11,
        y: 6,
        owner: BulletOwner::Player,
    }); // x+1, moves to y=5
    let s2 = tick(&s, &mut seeded_rng());
    assert!(s2.enemies.is_empty());
}

#[test]
fn tick_player_bullet_misses_enemy_outside_box() {
    let mut s = make_state();
    s.frame = 1;
    s.enemies.push(Enemy {
        x: 10,
        y: 5,
        kind: EnemyKind::Spacecraft,
    });
    s.bullets.push(Bullet {
        x: 12,
        y: 5,
        owner: BulletOwner::Player,
    }); // x+2, outside
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.enemies.len(), 1);
}

#[test]
fn tick_player_bullet_hits_enemy_second_row() {
    // Bounding box is 2-tall: enemy.y+1 also hits
    let mut s = make_state();
    s.frame = 1;
    s.enemies.push(Enemy {
        x: 10,
        y: 5,
        kind: EnemyKind::Spacecraft,
    });
    s.bullets.push(Bullet {
        x: 10,
        y: 6,
        owner: BulletOwner::Player,
    }); // y+1
    let s2 = tick(&s, &mut seeded_rng());
    assert!(s2.enemies.is_empty());
}

#[test]
fn tick_octopus_scores_150() {
    let mut s = make_state();
    s.frame = 1;
    s.enemies.push(Enemy {
        x: 10,
        y: 5,
        kind: EnemyKind::Octopus,
    });
    s.bullets.push(Bullet {
        x: 10,
        y: 6,
        owner: BulletOwner::Player,
    }); // moves to y=5
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
    s.bullets.push(Bullet {
        x: 20,
        y: 15,
        owner: BulletOwner::Enemy,
    }); // moves to y=16
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2);
}

#[test]
fn tick_player_loses_life_on_enemy_contact() {
    let mut s = make_state(); // player.y = 16
    s.frame = 1;
    s.enemies.push(Enemy {
        x: 5,
        y: 16,
        kind: EnemyKind::Spacecraft,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2);
}

#[test]
fn tick_game_over_when_lives_reach_zero() {
    let mut s = make_state();
    s.player.lives = 1;
    s.frame = 1;
    s.bullets.push(Bullet {
        x: 20,
        y: 15,
        owner: BulletOwner::Enemy,
    }); // moves to y=16
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 0);
    assert_eq!(s2.status, GameStatus::GameOver);
}

#[test]
fn tick_no_game_over_when_lives_above_zero() {
    let mut s = make_state();
    s.player.lives = 2;
    s.frame = 1;
    s.bullets.push(Bullet {
        x: 20,
        y: 15,
        owner: BulletOwner::Enemy,
    }); // moves to y=16
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 1);
    assert_eq!(s2.status, GameStatus::Playing);
}

#[test]
fn tick_lives_saturate_at_zero() {
    let mut s = make_state();
    s.player.lives = 0;
    s.frame = 1;
    s.bullets.push(Bullet {
        x: 20,
        y: 16,
        owner: BulletOwner::Enemy,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 0); // saturating_sub, no underflow
}

#[test]
fn tick_score_saturates_at_u32_max() {
    let mut s = make_state();
    s.score = u32::MAX;
    s.frame = 1;
    s.enemies.push(Enemy {
        x: 10,
        y: 5,
        kind: EnemyKind::Spacecraft,
    });
    s.bullets.push(Bullet {
        x: 10,
        y: 6,
        owner: BulletOwner::Player,
    }); // moves to y=5 -> kill worth +100
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.score, u32::MAX, "score must saturate at u32::MAX");
}

// ── tick — enemy bullet hitbox: 3-wide × 2-tall ──────────────────────────────
//
// Player sprite:   ▲        ← row player.y     (tip, 1 col wide)
//                 /█\       ← row player.y+1   (fuselage+wings, 3 cols wide)
//
// Hitbox covers both rows at x ± 1.

#[test]
fn tick_enemy_bullet_hits_player_fuselage_center() {
    // Bullet at (player.x, player.y) → moves to (player.x, player.y+1) = fuselage center
    let mut s = make_state(); // player at (20, 16)
    s.frame = 1;
    s.bullets.push(Bullet {
        x: 20,
        y: 16,
        owner: BulletOwner::Enemy,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2);
}

#[test]
fn tick_enemy_bullet_hits_player_left_wing() {
    // Bullet at (player.x-1, player.y) → moves to (player.x-1, player.y+1) = left wing
    let mut s = make_state();
    s.frame = 1;
    s.bullets.push(Bullet {
        x: 19,
        y: 16,
        owner: BulletOwner::Enemy,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2);
}

#[test]
fn tick_enemy_bullet_hits_player_right_wing() {
    // Bullet at (player.x+1, player.y) → moves to (player.x+1, player.y+1) = right wing
    let mut s = make_state();
    s.frame = 1;
    s.bullets.push(Bullet {
        x: 21,
        y: 16,
        owner: BulletOwner::Enemy,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2);
}

#[test]
fn tick_enemy_bullet_hits_player_tip_off_center() {
    // Bullet at (player.x-1, player.y-1) → moves to (player.x-1, player.y) = tip row, x-1
    let mut s = make_state();
    s.frame = 1;
    s.bullets.push(Bullet {
        x: 19,
        y: 15,
        owner: BulletOwner::Enemy,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2);
}

#[test]
fn tick_enemy_bullet_misses_player_too_far() {
    // Bullet at (player.x+2, player.y-1) → moves to (player.x+2, player.y): x=22, outside ±1
    let mut s = make_state();
    s.frame = 1;
    s.bullets.push(Bullet {
        x: 22,
        y: 15,
        owner: BulletOwner::Enemy,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 3);
}

#[test]
fn tick_enemy_bullet_consumed_on_hit() {
    // Bullet that hits the player should be removed from the field
    let mut s = make_state();
    s.frame = 1;
    s.bullets.push(Bullet {
        x: 20,
        y: 15,
        owner: BulletOwner::Enemy,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert!(s2.bullets.iter().all(|b| b.owner != BulletOwner::Enemy));
}

// ── tick — enemy spawn ────────────────────────────────────────────────────────

#[test]
fn tick_enemy_spawns_on_interval() {
    // Easy spawn_rate = 130; frame 129 → next frame 130 → spawn
    let mut s = make_state();
    s.frame = 129;
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

// ── Enemy contact: remove on reach ───────────────────────────────────────────

#[test]
fn tick_enemy_contact_costs_one_life() {
    let mut s = make_state();
    s.enemies.push(Enemy {
        x: s.player.x,
        y: s.player.y,
        kind: EnemyKind::Spacecraft,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2, "one life lost on contact");
}

#[test]
fn tick_enemy_contact_removes_enemy() {
    let mut s = make_state();
    s.enemies.push(Enemy {
        x: s.player.x,
        y: s.player.y,
        kind: EnemyKind::Spacecraft,
    });
    let s2 = tick(&s, &mut seeded_rng());
    assert!(
        s2.enemies.is_empty(),
        "enemy removed after crashing into player"
    );
}

#[test]
fn tick_enemy_contact_no_repeated_damage() {
    // After the enemy is removed on contact, subsequent ticks deal no further damage.
    let mut s = make_state();
    s.enemies.push(Enemy {
        x: s.player.x,
        y: s.player.y,
        kind: EnemyKind::Spacecraft,
    });
    let s2 = tick(&s, &mut seeded_rng());
    let s3 = tick(&s2, &mut seeded_rng());
    assert_eq!(s2.player.lives, 2, "one life lost");
    assert_eq!(s3.player.lives, 2, "no further damage next frame");
}
