//! Game rules systems.
//!
//! Handles trigger detection, arrival events, and game over conditions.

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::bevy::{GameOverEvent, Marble, MarbleArrivedEvent, MarbleGameState, TriggerZone};

/// System to check for marble-trigger collisions.
pub fn check_trigger_arrivals(
    mut collision_events: MessageReader<CollisionEvent>,
    marbles: Query<(Entity, &Marble)>,
    triggers: Query<(Entity, &TriggerZone)>,
    mut arrival_events: MessageWriter<MarbleArrivedEvent>,
    mut game_state: ResMut<MarbleGameState>,
) {
    for event in collision_events.read() {
        let CollisionEvent::Started(e1, e2, _) = event else {
            continue;
        };

        // Check both orderings
        let (marble_entity, marble, trigger) = if let (Ok((entity, marble)), Ok((_, trigger))) =
            (marbles.get(*e1), triggers.get(*e2))
        {
            (entity, marble, trigger)
        } else if let (Ok((entity, marble)), Ok((_, trigger))) = (marbles.get(*e2), triggers.get(*e1))
        {
            (entity, marble, trigger)
        } else {
            continue;
        };

        // Skip if already eliminated
        if marble.eliminated {
            continue;
        }

        // Check if player already in arrival order
        if game_state.arrival_order.contains(&marble.owner_id) {
            continue;
        }

        // Record arrival
        game_state.arrival_order.push(marble.owner_id);

        // Send event
        arrival_events.write(MarbleArrivedEvent {
            player_id: marble.owner_id,
            marble_entity,
            trigger_action: trigger.action.clone(),
            trigger_index: trigger.trigger_index,
        });
    }
}

/// System to handle marble arrival events.
pub fn handle_marble_arrivals(
    mut commands: Commands,
    mut events: MessageReader<MarbleArrivedEvent>,
    mut marbles: Query<&mut Marble>,
) {
    for event in events.read() {
        if event.trigger_action == "gamerule" {
            // Remove marble entirely
            commands.entity(event.marble_entity).despawn();
        } else {
            // Just mark as eliminated and disable physics
            if let Ok(mut marble) = marbles.get_mut(event.marble_entity) {
                marble.eliminated = true;
            }
            // Disable physics by removing components
            commands
                .entity(event.marble_entity)
                .remove::<RigidBody>()
                .remove::<Collider>();
        }
    }
}

/// System to check for game over condition.
pub fn check_game_over(
    marbles: Query<&Marble>,
    game_state: Res<MarbleGameState>,
    mut game_over_events: MessageWriter<GameOverEvent>,
) {
    // Count active marbles
    let active_count = marbles.iter().filter(|m| !m.eliminated).count();

    // Game is over when all players have arrived
    if active_count == 0 && !game_state.players.is_empty() && !game_state.arrival_order.is_empty() {
        game_over_events.write(GameOverEvent);
    }
}
