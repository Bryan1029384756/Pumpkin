use std::sync::Arc;

use crate::entity::player::Player;
use crate::item::pumpkin_item::{ItemMetadata, PumpkinItem};
use crate::server::Server;
use async_trait::async_trait;
use pumpkin_data::block::{Block, BlockState};
use pumpkin_data::item::Item;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::block::BlockDirection;
use pumpkin_util::math::vector3::Vector3;

pub struct BucketItem;

impl ItemMetadata for BucketItem {
    const IDS: &'static [u16] = &[
        Item::BUCKET.id, 
        Item::WATER_BUCKET.id, 
        Item::LAVA_BUCKET.id,
        Item::POWDER_SNOW_BUCKET.id,
        Item::MILK_BUCKET.id
    ];
}

#[async_trait]
impl PumpkinItem for BucketItem {
    async fn use_on_block(
        &self,
        item: &Item,
        player: &Player,
        location: BlockPos,
        block: &Block,
        server: &Server,
    ) {
        // Only handle fluid buckets, not empty buckets or milk
        if item.id != Item::WATER_BUCKET.id && item.id != Item::LAVA_BUCKET.id && item.id != Item::POWDER_SNOW_BUCKET.id {
            return;
        }

        log::info!("Player using bucket: {} at position {:?}", item.registry_key, location.0);
        
        let world = player.world().await;
        
        // Determine face direction using ray casting
        let face_direction = if let Some((target_block_pos, _)) = player.get_targeted_block(5.0).await {
            // If we're looking at the same block as the interaction location
            if target_block_pos == location {
                // Calculate which face is being looked at
                let entity = &player.living_entity.entity;
                let position = entity.pos.load();
                let eye_position = Vector3::new(
                    position.x,
                    position.y + f64::from(entity.standing_eye_height),
                    position.z,
                );
                
                // Get look direction as f64
                let pitch = f64::from(entity.pitch.load());
                let yaw = f64::from(entity.yaw.load());
                let pitch_rad = pitch.to_radians();
                let yaw_rad = yaw.to_radians();
                
                // Calculate direction components
                let dx = -yaw_rad.sin() * pitch_rad.cos();
                let dy = -pitch_rad.sin();
                let dz = yaw_rad.cos() * pitch_rad.cos();
                
                // Determine which face based on the dominant direction
                let loc_f64 = location.to_f64();
                let target_center = Vector3::new(
                    loc_f64.x + 0.5,
                    loc_f64.y + 0.5,
                    loc_f64.z + 0.5
                );
                
                // Vector from block center to eye position
                let to_eye = Vector3::new(
                    eye_position.x - target_center.x,
                    eye_position.y - target_center.y,
                    eye_position.z - target_center.z
                );
                
                // The face is the one most aligned with the to_eye vector
                let abs_x = to_eye.x.abs();
                let abs_y = to_eye.y.abs();
                let abs_z = to_eye.z.abs();
                
                if abs_x > abs_y && abs_x > abs_z {
                    if to_eye.x > 0.0 { BlockDirection::East } else { BlockDirection::West }
                } else if abs_y > abs_x && abs_y > abs_z {
                    if to_eye.y > 0.0 { BlockDirection::Up } else { BlockDirection::Down }
                } else {
                    if to_eye.z > 0.0 { BlockDirection::South } else { BlockDirection::North }
                }
            } else {
                // Default to UP if the targeted block is different from the interaction block
                BlockDirection::Up
            }
        } else {
            // Default to UP if no block is targeted
            BlockDirection::Up
        };

        let target_pos = location.offset(face_direction.to_offset());
        
        // Check if the target position can be replaced with fluid
        if let Ok(target_state) = world.get_block_state(&target_pos).await {
            if !target_state.replaceable && !target_state.air {
                // Can't place fluid there
                return;
            }
            
            // Determine which block to place
            let (block_to_place, sound, is_water) = match item.id {
                id if id == Item::WATER_BUCKET.id => {
                    (Block::WATER.default_state_id, Sound::ItemBucketEmpty, true)
                },
                id if id == Item::LAVA_BUCKET.id => {
                    (Block::LAVA.default_state_id, Sound::ItemBucketEmptyLava, false)
                },
                id if id == Item::POWDER_SNOW_BUCKET.id => {
                    (Block::POWDER_SNOW.default_state_id, Sound::ItemBucketEmptyPowderSnow, false)
                },
                _ => return, // Shouldn't happen due to earlier check
            };
            
            // Place the fluid or block
            log::info!("Placing fluid/block with state ID: {} at position {:?}", block_to_place, target_pos.0);
            world.set_block_state(&target_pos, block_to_place).await;
            
            // Trigger fluid updates for water and lava
            if item.id == Item::WATER_BUCKET.id || item.id == Item::LAVA_BUCKET.id {
                let mut fluid_manager = world.fluid_manager.lock().await;
                
                // IMPORTANT: When adding a source from a bucket, use a special flag
                // to indicate it's a player-placed source that should be more stable
                fluid_manager.add_fluid_source(&world, server, target_pos, is_water).await;
                
                // Schedule immediate updates for neighboring blocks to ensure proper flow
                for direction in BlockDirection::all() {
                    let adjacent_pos = target_pos.offset(direction.to_offset());
                    if let Ok(adjacent_state_id) = world.get_block_state_id(&adjacent_pos).await {
                        if fluid_manager.is_water(adjacent_state_id) {
                            // Use high priority (3) for player bucket placed water
                            fluid_manager.schedule_update(adjacent_pos, adjacent_state_id, 0, 3);
                        }
                    }
                }
            }
            
            // Play appropriate sound
            world.play_sound(sound, SoundCategory::Blocks, &target_pos.to_f64()).await;
            
            // Update player's held item to empty bucket if not in creative mode
            if player.gamemode.load() != pumpkin_util::GameMode::Creative {
                let mut inventory = player.inventory().lock().await;
                let selected_slot = inventory.get_selected_slot();
                
                if let Some(stack) = inventory.held_item_mut() {
                    stack.item = Item::BUCKET;
                    let stack_clone = stack.clone();
                    
                    // Drop the mutable borrow before calling update_single_slot
                    drop(stack);
                    
                    player.update_single_slot(&mut inventory, selected_slot, stack_clone).await;
                }
            }
        }
    }
}