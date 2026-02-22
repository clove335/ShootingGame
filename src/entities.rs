/// All game entity types — pure data, no logic.

#[derive(Clone, Debug, PartialEq)]
pub enum EnemyKind {
    /// Shocking-green spacecraft
    Spacecraft,
    /// Red octopus
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

/// The entire game state.  Cloneable so pure update functions can
/// return a new copy without mutating the original.
#[derive(Clone, Debug)]
pub struct EntireGameStateInfo {
    pub player: Player,
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub score: u32,
    pub level: Level,
    pub status: GameStatus,
    pub frame: u64,
    pub width: u16,
    pub height: u16,
}
