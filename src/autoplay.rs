use crate::compute::{move_player_left, move_player_right, player_shoot};
use crate::entities::{BulletOwner, EntireGameStateInfo};

pub fn update_autoplay(state: &EntireGameStateInfo) -> EntireGameStateInfo {
    let mut current_state = state.clone();

    // 1. Simple heuristic: find the lowest enemy and align with it.
    // If no enemies, stay put or move to center.
    let target_x = if let Some(target) = state.enemies.iter().max_by_key(|e| e.y) {
        target.x
    } else if let Some(bonus) = state.bonus_items.iter().max_by_key(|b| b.y) {
        bonus.x
    } else {
        (state.width / 2) as i32
    };

    if current_state.player.x < target_x {
        current_state = move_player_right(&current_state);
    } else if current_state.player.x > target_x {
        current_state = move_player_left(&current_state);
    }

    // 2. Simple heuristic: Avoid enemy bullets that are directly above.
    let dangerous_bullet = state.bullets.iter().find(|b| {
        b.owner == BulletOwner::Enemy
            && (b.x - state.player.x).abs() <= 1
            && b.y < state.player.y
            && b.y > state.player.y - 5
    });

    if let Some(bullet) = dangerous_bullet {
        // Try to dodge
        if state.player.x <= bullet.x && state.player.x > 1 {
            current_state = move_player_left(&current_state);
        } else if state.player.x >= bullet.x && state.player.x < (state.width as i32 - 2) {
            current_state = move_player_right(&current_state);
        }
    }

    // 3. Always shoot if an enemy is in front or randomly.
    let enemy_in_front = state
        .enemies
        .iter()
        .any(|e| (e.x - state.player.x).abs() <= 2);
    #[allow(clippy::manual_is_multiple_of)]
    let should_shoot = enemy_in_front || state.frame % 5 == 0;

    if should_shoot {
        current_state = player_shoot(&current_state);
    }

    current_state
}
