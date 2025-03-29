use pumpkin_macros::Event;
use pumpkin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player spawns after death.
///
/// This event cannot be cancelled, but you can modify the spawn position
/// and other properties through this event.
///
/// This event contains information about the player respawning and their spawn location.
#[derive(Event, Clone)]
pub struct PlayerSpawnLocationEvent {
    /// The player who is spawning.
    pub player: Arc<Player>,

    /// The position where the player will spawn.
    pub spawn_position: Vector3<f64>,

    /// The yaw angle (horizontal rotation) after spawn.
    pub yaw: f32,

    /// The pitch angle (vertical rotation) after spawn.
    pub pitch: f32,
}

impl PlayerSpawnLocationEvent {
    /// Creates a new instance of `PlayerSpawnLocationEvent`.
    ///
    /// # Arguments
    /// - `player`: A reference to the player who is respawning.
    /// - `spawn_position`: The position where the player will spawn.
    /// - `yaw`: The yaw angle (horizontal rotation) after spawn.
    /// - `pitch`: The pitch angle (vertical rotation) after spawn.
    ///
    /// # Returns
    /// A new instance of `PlayerSpawnLocationEvent`.
    pub fn new(player: Arc<Player>, spawn_position: Vector3<f64>, yaw: f32, pitch: f32) -> Self {
        Self {
            player,
            spawn_position,
            yaw,
            pitch,
        }
    }

    #[must_use]
    pub fn get_spawn_position(&self) -> Vector3<f64> {
        self.spawn_position
    }

    #[must_use]
    pub fn get_yaw(&self) -> f32 {
        self.yaw
    }

    #[must_use]
    pub fn get_pitch(&self) -> f32 {
        self.pitch
    }

    pub fn set_spawn_position(&mut self, spawn_position: Vector3<f64>) {
        self.spawn_position = spawn_position;
    }

    pub fn set_yaw(&mut self, yaw: f32) {
        self.yaw = yaw;
    }

    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch;
    }
}

impl PlayerEvent for PlayerSpawnLocationEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
