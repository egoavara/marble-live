//! Follow camera systems.
//!
//! Systems for following specific marbles or the leading marble.

use bevy::prelude::*;

use crate::bevy::{CameraMode, GameCamera, MainCamera, Marble};
use crate::marble::PlayerId;

/// System to update camera when following a specific player's marble.
///
/// Only runs when `CameraMode::FollowTarget(player_id)` is active.
pub fn update_follow_target(
    mut cameras: Query<&mut GameCamera, With<MainCamera>>,
    marbles: Query<(&Marble, &Transform)>,
) {
    for mut game_camera in cameras.iter_mut() {
        let CameraMode::FollowTarget(target_player_id) = game_camera.mode else {
            continue;
        };

        // Find the marble belonging to the target player
        let target_marble = marbles
            .iter()
            .find(|(marble, _)| marble.owner_id == target_player_id && !marble.eliminated);

        if let Some((_, transform)) = target_marble {
            let marble_pos = transform.translation.truncate();
            game_camera.target_position = marble_pos;
        }
        // If marble not found (eliminated or not spawned), keep current position
    }
}

/// System to update camera when following the leading marble.
///
/// The leader is determined by the highest Y position (progressed furthest).
/// Uses hysteresis to prevent rapid switching between marbles:
/// - A new leader must be at least `leader_switch_margin` ahead
/// - After switching, there's a cooldown period before another switch
pub fn update_follow_leader(mut cameras: Query<&mut GameCamera, With<MainCamera>>, marbles: Query<(&Marble, &Transform)>) {
    for mut game_camera in cameras.iter_mut() {
        if game_camera.mode != CameraMode::FollowLeader {
            continue;
        }

        // Decrement cooldown
        if game_camera.current_cooldown > 0 {
            game_camera.current_cooldown -= 1;
        }

        // Find all active marbles with their positions
        let active_marbles: Vec<(PlayerId, Vec2)> = marbles
            .iter()
            .filter(|(marble, _)| !marble.eliminated)
            .map(|(marble, transform)| (marble.owner_id, transform.translation.truncate()))
            .collect();

        if active_marbles.is_empty() {
            // No active marbles, keep current position
            continue;
        }

        // Find the marble with highest Y (leader)
        let (potential_leader_id, leader_pos) = active_marbles
            .iter()
            .max_by(|(_, a), (_, b)| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, pos)| (*id, *pos))
            .unwrap();

        // Check if we should switch leaders
        let should_switch = match game_camera.current_leader {
            None => true, // No current leader, always switch
            Some(current_id) => {
                // Only switch if cooldown is 0 and new leader is significantly ahead
                if game_camera.current_cooldown > 0 {
                    false
                } else if potential_leader_id == current_id {
                    false // Same leader, no switch needed
                } else {
                    // Find current leader's position
                    let current_pos = active_marbles
                        .iter()
                        .find(|(id, _)| *id == current_id)
                        .map(|(_, pos)| *pos);

                    match current_pos {
                        Some(current) => {
                            // Only switch if new leader is margin ahead
                            leader_pos.y > current.y + game_camera.leader_switch_margin
                        }
                        None => true, // Current leader eliminated, switch immediately
                    }
                }
            }
        };

        if should_switch && Some(potential_leader_id) != game_camera.current_leader {
            game_camera.current_leader = Some(potential_leader_id);
            game_camera.current_cooldown = game_camera.leader_cooldown;
        }

        // Update target position to current leader
        if let Some(leader_id) = game_camera.current_leader {
            if let Some((_, pos)) = active_marbles.iter().find(|(id, _)| *id == leader_id) {
                game_camera.target_position = *pos;
            } else {
                // Leader was eliminated, follow the new leader
                game_camera.target_position = leader_pos;
                game_camera.current_leader = Some(potential_leader_id);
            }
        }
    }
}
