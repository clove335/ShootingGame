/// All game entity types — pure data, no logic.

#[derive(Clone, Debug, PartialEq)]
pub enum EnemyKind {
    Spacecraft,
    Octopus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Level {
    Easy,
    Medium,
    Hard,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GameStatus {
    Playing,
    GameOver,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BonusKind {
    /// 3-way spread shot (straight up) for POWER_UP_DURATION frames.
    SpreadShot,
    /// Instantly adds one life (max 5).
    ExtraLife,
    /// Raises the on-screen bullet cap to 6 for POWER_UP_DURATION frames.
    RapidFire,
    /// 4-way angled flame burst for POWER_UP_DURATION frames.
    /// Each flame fires at ±18° and ±54° from vertical (36° apart).
    FlameBurst,
    /// Slow-moving firebomb for POWER_UP_DURATION frames.
    /// Explodes on enemy contact or when it reaches the top, damaging
    /// every enemy within EXPLOSION_RADIUS cells.
    Firebomb,
}

// ── Bonus items ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct BonusItem {
    pub x: i32,
    pub y: i32,
    pub kind: BonusKind,
}

// ── Projectiles ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum BulletOwner {
    Player,
    Enemy,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub x: i32,
    pub y: i32,
    pub owner: BulletOwner,
}

/// A player bullet that travels diagonally (used by the FlameBurst power-up).
/// Positions are stored as floats so sub-column angles stay smooth.
#[derive(Clone, Debug)]
pub struct FlameBullet {
    /// Horizontal position (fractional columns).
    pub x: f32,
    /// Vertical position (fractional rows).
    pub y: f32,
    /// Horizontal velocity added each frame (positive = rightward).
    /// Vertical velocity is always −1.0 (one row upward per frame).
    pub vx: f32,
}

/// A slow-moving explosive projectile (used by the Firebomb power-up).
#[derive(Clone, Debug)]
pub struct FirebombProj {
    pub x: i32,
    pub y: i32,
    /// Frames until automatic detonation even without hitting anything.
    pub fuse: u32,
}

/// A brief visual explosion rendered for a few frames after a firebomb detonates.
#[derive(Clone, Debug)]
pub struct Explosion {
    pub x: i32,
    pub y: i32,
    /// Remaining frames to display.
    pub frames: u32,
}

// ── Player & enemy ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Player {
    pub x: i32,
    pub y: i32,
    pub lives: u32,
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub x: i32,
    pub y: i32,
    pub kind: EnemyKind,
}

// ── Master game state ─────────────────────────────────────────────────────────

/// The entire game state.  Cloneable so pure update functions can
/// return a new copy without mutating the original.
#[derive(Clone, Debug)]
pub struct EntireGameStateInfo {
    pub player: Player,
    pub enemies: Vec<Enemy>,
    /// Standard (straight-moving) bullets from player and enemies.
    pub bullets: Vec<Bullet>,
    /// Diagonally-moving flame bullets fired during FlameBurst.
    pub flame_bullets: Vec<FlameBullet>,
    /// Slow firebomb projectiles fired during the Firebomb power-up.
    pub firebombs: Vec<FirebombProj>,
    /// Short-lived explosion visuals after a firebomb detonates.
    pub explosions: Vec<Explosion>,
    /// Bonus power-up items currently falling through the play area.
    pub bonus_items: Vec<BonusItem>,
    /// Active power-up and the number of frames remaining, if any.
    pub active_power_up: Option<(BonusKind, u32)>,
    pub score: u32,
    /// The highest score seen so far (updated live during play).
    pub high_score: u32,
    pub level: Level,
    pub status: GameStatus,
    pub frame: u64,
    pub width: u16,
    pub height: u16,
}
