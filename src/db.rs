use rusqlite::{params, Connection, Result};
use std::path::PathBuf;

use shooting_game::entities::Level;

fn db_path() -> PathBuf {
    PathBuf::from("shooting_game.db")
}

fn level_str(level: &Level) -> &'static str {
    match level {
        Level::Easy => "easy",
        Level::Medium => "medium",
        Level::Hard => "hard",
        Level::Extreme => "extreme",
    }
}

pub fn open() -> Option<Connection> {
    let conn = Connection::open(db_path()).ok()?;
    init(&conn).ok()?;
    Some(conn)
}

fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS top_scores (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            username    TEXT    NOT NULL,
            difficulty  TEXT    NOT NULL,
            points      INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),
            deleted_at  TEXT,
            UNIQUE(username, difficulty)
        );
        CREATE TABLE IF NOT EXISTS scores (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            username    TEXT    NOT NULL,
            difficulty  TEXT    NOT NULL,
            points      INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
            deleted_at  TEXT
        );",
    )
}

/// Insert one completed game into `scores`.
pub fn insert_score(conn: &Connection, username: &str, level: &Level, points: u32) -> Result<()> {
    conn.execute(
        "INSERT INTO scores (username, difficulty, points) VALUES (?1, ?2, ?3)",
        params![username, level_str(level), points],
    )?;
    Ok(())
}

/// Upsert into `top_scores`: insert on first game, update only if the new score is higher.
pub fn upsert_top_score(
    conn: &Connection,
    username: &str,
    level: &Level,
    points: u32,
) -> Result<()> {
    conn.execute(
        "INSERT INTO top_scores (username, difficulty, points) VALUES (?1, ?2, ?3)
         ON CONFLICT(username, difficulty) DO UPDATE SET
             points     = MAX(points, excluded.points),
             updated_at = CASE WHEN excluded.points > points
                               THEN datetime('now') ELSE updated_at END",
        params![username, level_str(level), points],
    )?;
    Ok(())
}

/// Best score across all difficulties (used for the menu display).
pub fn load_best_score(conn: &Connection) -> u32 {
    conn.query_row(
        "SELECT COALESCE(MAX(points), 0) FROM top_scores WHERE deleted_at IS NULL",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|v| v as u32)
    .unwrap_or(0)
}
