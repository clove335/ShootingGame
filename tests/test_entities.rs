use shooting_game::entities::*;

#[test]
fn entity_clone_and_eq() {
    // Enums derive PartialEq — equality comparisons must work
    assert_eq!(EnemyKind::Spacecraft, EnemyKind::Spacecraft);
    assert_ne!(EnemyKind::Spacecraft, EnemyKind::Octopus);
    assert_eq!(Level::Easy, Level::Easy);
    assert_ne!(Level::Easy, Level::Hard);
    assert_eq!(GameStatus::Playing, GameStatus::Playing);
    assert_ne!(GameStatus::Playing, GameStatus::GameOver);
    assert_eq!(BulletOwner::Player, BulletOwner::Player);
    assert_ne!(BulletOwner::Player, BulletOwner::Enemy);

    // Clone must produce an equal value
    let kind = EnemyKind::Octopus;
    assert_eq!(kind.clone(), EnemyKind::Octopus);
}

#[test]
fn game_state_clone_is_independent() {
    let original = EntireGameStateInfo {
        player: Player { x: 20, y: 16, lives: 3 },
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
    };
    let mut cloned = original.clone();

    // Mutating the clone must not affect the original
    cloned.player.x = 99;
    cloned.score = 999;
    cloned.enemies.push(Enemy { x: 5, y: 5, kind: EnemyKind::Spacecraft });

    assert_eq!(original.player.x, 20);
    assert_eq!(original.score, 0);
    assert!(original.enemies.is_empty());
}
