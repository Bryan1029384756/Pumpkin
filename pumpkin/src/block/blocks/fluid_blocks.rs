// Updated fluid_blocks.rs with correct schedule_update calls

use crate::block::pumpkin_block::{BlockMetadata, PumpkinBlock};
use crate::block::registry::BlockActionResult;
use crate::entity::player::Player;
use crate::server::Server;
use crate::world::World;
use async_trait::async_trait;
use pumpkin_data::block::{Block, BlockState, HorizontalFacing};
use pumpkin_data::item::Item;
use pumpkin_data::tag::RegistryKey;
use pumpkin_data::tag::get_tag_values;
use pumpkin_macros::pumpkin_block;
use pumpkin_protocol::server::play::SUseItemOn;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::block::BlockDirection;
use std::sync::Arc;


fn direction_to_string(dir: &BlockDirection) -> &'static str {
    match dir {
        BlockDirection::Up => "Up",
        BlockDirection::Down => "Down",
        BlockDirection::North => "North", 
        BlockDirection::South => "South",
        BlockDirection::East => "East",
        BlockDirection::West => "West",
    }
}

#[pumpkin_block("minecraft:water")]
pub struct WaterBlock;

#[async_trait]
impl PumpkinBlock for WaterBlock {
    async fn broken(
        &self,
        block: &Block,
        player: &Player,
        location: BlockPos,
        server: &Server,
        world: Arc<World>,
        _state: BlockState,
    ) {
        // Remove the water source and trigger updates
        let mut fluid_manager = world.fluid_manager.lock().await;
        fluid_manager.remove_fluid(&world, server, location).await;
    }

    async fn on_place(
        &self,
        server: &Server,
        world: &World,
        block: &Block,
        _face: &BlockDirection,
        block_pos: &BlockPos,
        _use_item_on: &SUseItemOn,
        _player_direction: &HorizontalFacing,
        _other: bool,
    ) -> u16 {
        log::info!("Placing water block at {:?}", block_pos.0);
        
        // Find the source block state ID
        let source_state_id = block.default_state_id;
        
        // Schedule an immediate update for fluid mechanics
        let mut fluid_manager = world.fluid_manager.lock().await;
        fluid_manager.add_fluid_source(world, server, *block_pos, true).await;
        
        // Return the source block state ID
        source_state_id
    }

    async fn on_neighbor_update(
        &self,
        server: &Server,
        world: &World,
        block: &Block,
        block_pos: &BlockPos,
        source_face: &BlockDirection,
        source_block_pos: &BlockPos,
    ) {
        // Get the current state ID of this water block
        if let Ok(state_id) = world.get_block_state_id(block_pos).await {
            // Skip if block is no longer water (might have been replaced)
            let mut fluid_manager = world.fluid_manager.lock().await;
            if !fluid_manager.is_water(state_id) {
                return;
            }
            
            // Schedule an update for the water with normal priority (1)
            fluid_manager.schedule_update(*block_pos, state_id, 1, 1);
            
            // Also check if the block below is now empty (can fall into it)
            let below_pos = block_pos.offset(BlockDirection::Down.to_offset());
            if let Ok(below_state) = world.get_block_state(&below_pos).await {
                if below_state.replaceable || below_state.air {
                    // If the block below can be filled, prioritize this update (higher priority 2)
                    fluid_manager.schedule_update(*block_pos, state_id, 0, 2);
                }
            }
        }
    }
}

#[pumpkin_block("minecraft:lava")]
pub struct LavaBlock;

#[async_trait]
impl PumpkinBlock for LavaBlock {
    async fn broken(
        &self,
        block: &Block,
        player: &Player,
        location: BlockPos,
        server: &Server,
        world: Arc<World>,
        _state: BlockState,
    ) {
        // Remove the lava source and trigger updates
        let mut fluid_manager = world.fluid_manager.lock().await;
        fluid_manager.remove_fluid(&world, server, location).await;
    }

    async fn on_place(
        &self,
        server: &Server,
        world: &World,
        block: &Block,
        _face: &BlockDirection,
        block_pos: &BlockPos,
        _use_item_on: &SUseItemOn,
        _player_direction: &HorizontalFacing,
        _other: bool,
    ) -> u16 {
        log::info!("Placing lava block at {:?}", block_pos.0);
        
        // Find the source block state ID  
        let source_state_id = block.default_state_id;
        
        // Schedule an immediate update for fluid mechanics
        let mut fluid_manager = world.fluid_manager.lock().await;
        fluid_manager.add_fluid_source(world, server, *block_pos, false).await;
        
        // Return the source block state ID
        source_state_id
    }

    async fn on_neighbor_update(
        &self,
        server: &Server,
        world: &World,
        block: &Block,
        block_pos: &BlockPos,
        source_face: &BlockDirection,
        source_block_pos: &BlockPos,
    ) {
        // The same logic as WaterBlock, but check for lava blocks
        if let Ok(state_id) = world.get_block_state_id(block_pos).await {
            let mut fluid_manager = world.fluid_manager.lock().await;
            
            // Check if it's lava (you would need to add an is_lava method to FluidManager)
            if !fluid_manager.is_water(state_id) { // Replace with is_lava check when you implement lava
                return;
            }
            
            // Schedule update - lava is slower, so use a longer delay with normal priority (1)
            fluid_manager.schedule_update(*block_pos, state_id, 2, 1);
            
            // Check if it can fall down
            let below_pos = block_pos.offset(BlockDirection::Down.to_offset());
            if let Ok(below_state) = world.get_block_state(&below_pos).await {
                if below_state.replaceable || below_state.air {
                    // Higher priority (2) for falling down
                    fluid_manager.schedule_update(*block_pos, state_id, 0, 2);
                }
            }
        }
    }
}

// Register fluid blocks in the block registry
pub fn register_fluid_blocks(manager: &mut crate::block::registry::BlockRegistry) {
    manager.register(WaterBlock);
    manager.register(LavaBlock);
}