use pumpkin_data::damage::DamageType;
use pumpkin_macros::Event;
use pumpkin_util::text::TextComponent;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player dies.
///
///
/// This event contains information about the player who died, the death message, and the damage source.
#[derive(Event, Clone)]
pub struct PlayerDeathEvent {
    /// The player who died.
    pub player: Arc<Player>,

    /// The death message to display to other players.
    pub death_message: TextComponent,

    /// The type of the damage that killed the player.
    pub damage_type: DamageType,
}

impl PlayerDeathEvent {
    /// Creates a new instance of `PlayerDeathEvent`.
    ///
    /// # Arguments
    /// - `player`: A reference to the player who died.
    /// - `death_message`: The message to display when the player dies.
    /// - `damage_type`: The source of the damage that killed the player, if any.
    ///
    /// # Returns
    /// A new instance of `PlayerDeathEvent`.
    pub fn new(
        player: Arc<Player>,
        death_message: TextComponent,
        damage_type: DamageType,
    ) -> Self {
        Self {
            player,
            death_message,
            damage_type,
        }
    }
}

impl PlayerEvent for PlayerDeathEvent {
    
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}