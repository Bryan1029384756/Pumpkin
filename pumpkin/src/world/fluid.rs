use std::sync::Arc;
use std::collections::{VecDeque, HashSet, HashMap};

use pumpkin_protocol::client::play::CBlockUpdate;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_world::block::BlockDirection;
use pumpkin_world::block::registry::get_block_by_state_id;

use crate::{world::World, server::Server};

// Constants for fluid behavior matching vanilla mechanics
const HORIZONTAL_MAX_FLOW_DISTANCE: i32 = 7;
const FLUID_TICK_RATE: u32 = 5;
const MAX_DOWNWARD_PATH_DISTANCE: i32 = 4;
const DEFAULT_FLOW_WEIGHT: i32 = 1000;
const MAX_UPDATES_PER_TICK: usize = 256;

// Water block state IDs - Make sure these match your actual block state IDs
const WATER_SOURCE_STATE_ID: u16 = 86;
const WATER_LEVEL_1_STATE_ID: u16 = 93; // Level 1 water (furthest from source)
const WATER_LEVEL_2_STATE_ID: u16 = 92; // Level 2 water
const WATER_LEVEL_3_STATE_ID: u16 = 91; // Level 3 water
const WATER_LEVEL_4_STATE_ID: u16 = 90; // Level 4 water
const WATER_LEVEL_5_STATE_ID: u16 = 89; // Level 5 water
const WATER_LEVEL_6_STATE_ID: u16 = 88; // Level 6 water
const WATER_LEVEL_7_STATE_ID: u16 = 87; // Level 7 water (closest to source)

/// Store pending fluid updates to be processed in order
#[derive(Clone, Eq, PartialEq, Hash)]
struct FluidUpdate {
    position: BlockPos,
    fluid_state_id: u16,
    tick_scheduled: u32,
    priority: i32, // Higher priority = process first
}

/// Manages fluid mechanics in the world
pub struct FluidManager {
    pending_updates: HashMap<BlockPos, FluidUpdate>,
    current_tick: u32,
    batch_updates: Vec<(BlockPos, u16)>,
}

impl Default for FluidManager {
    fn default() -> Self {
        Self {
            pending_updates: HashMap::with_capacity(1024),
            current_tick: 0,
            batch_updates: Vec::with_capacity(256),
        }
    }
}

impl FluidManager {
    /// Create a new FluidManager
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Determine if a state ID is a water block
    pub fn is_water(&self, state_id: u16) -> bool {
        match state_id {
            WATER_SOURCE_STATE_ID |
            WATER_LEVEL_1_STATE_ID |
            WATER_LEVEL_2_STATE_ID |
            WATER_LEVEL_3_STATE_ID |
            WATER_LEVEL_4_STATE_ID |
            WATER_LEVEL_5_STATE_ID |
            WATER_LEVEL_6_STATE_ID |
            WATER_LEVEL_7_STATE_ID => true,
            _ => false
        }
    }

    /// Determine if a state ID is a source block
    fn is_source_block(&self, state_id: u16) -> bool {
        state_id == WATER_SOURCE_STATE_ID
    }

    /// Get the water level (1-8) from a state ID
    fn get_water_level(&self, state_id: u16) -> i32 {
        match state_id {
            WATER_SOURCE_STATE_ID => 8, // Sources are level 8
            WATER_LEVEL_7_STATE_ID => 7,
            WATER_LEVEL_6_STATE_ID => 6,
            WATER_LEVEL_5_STATE_ID => 5,
            WATER_LEVEL_4_STATE_ID => 4,
            WATER_LEVEL_3_STATE_ID => 3,
            WATER_LEVEL_2_STATE_ID => 2,
            WATER_LEVEL_1_STATE_ID => 1,
            _ => 0
        }
    }

    /// Get the state ID for a specific water level
    fn get_state_id_for_level(&self, level: i32) -> u16 {
        match level {
            8 => WATER_SOURCE_STATE_ID,
            7 => WATER_LEVEL_7_STATE_ID,
            6 => WATER_LEVEL_6_STATE_ID,
            5 => WATER_LEVEL_5_STATE_ID,
            4 => WATER_LEVEL_4_STATE_ID,
            3 => WATER_LEVEL_3_STATE_ID,
            2 => WATER_LEVEL_2_STATE_ID,
            1 => WATER_LEVEL_1_STATE_ID,
            _ => 0
        }
    }
    pub async fn add_fluid_source(&mut self, world: &World, server: &Server, position: BlockPos, is_water: bool) {
        if !is_water {
            return; // Only handle water in this implementation
        }
        
        // Set the block to water source
        world.set_block_state(&position, WATER_SOURCE_STATE_ID).await;
        
        // Schedule update with high priority
        self.schedule_update(position, WATER_SOURCE_STATE_ID, 0, 3);
    }

    pub async fn remove_fluid(&mut self, world: &World, server: &Server, position: BlockPos) {
        // Check if this is a source block before removal
        let is_source = if let Ok(state_id) = world.get_block_state_id(&position).await {
            self.is_source_block(state_id)
        } else {
            false
        };
        
        // Set to air
        world.set_block_state(&position, 0).await;
        
        // Schedule updates for adjacent blocks
        for direction in BlockDirection::all() {
            let adjacent_pos = position.offset(direction.to_offset());
            if let Ok(adjacent_state_id) = world.get_block_state_id(&adjacent_pos).await {
                if self.is_water(adjacent_state_id) {
                    let priority = if is_source { 2 } else { 1 };
                    self.schedule_update(adjacent_pos, adjacent_state_id, 0, priority);
                }
            }
        }
    }

    /// Handle block placement near water
    pub async fn handle_block_placed_near_water(&mut self, world: &World, server: &Server, position: BlockPos) {
        // Check adjacent blocks for water and schedule updates
        for direction in BlockDirection::all() {
            let adjacent_pos = position.offset(direction.to_offset());
            if let Ok(adjacent_state_id) = world.get_block_state_id(&adjacent_pos).await {
                if self.is_water(adjacent_state_id) {
                    // Higher priority for source blocks
                    let priority = if self.is_source_block(adjacent_state_id) { 2 } else { 1 };
                    self.schedule_update(adjacent_pos, adjacent_state_id, 0, priority);
                }
            }
        }
    }

    /// Schedule a fluid update without duplicates
    pub fn schedule_update(&mut self, position: BlockPos, fluid_state_id: u16, delay: u32, priority: i32) {
        // Only schedule if it's a fluid block or air that needs to be checked
        if fluid_state_id != 0 && !self.is_water(fluid_state_id) {
            return;
        }
        
        let actual_delay = FLUID_TICK_RATE + delay;
        let scheduled_tick = self.current_tick + actual_delay;
        
        // Store in a HashMap with position as the key to prevent duplicates
        let update = FluidUpdate {
            position,
            fluid_state_id,
            tick_scheduled: scheduled_tick,
            priority,
        };
        
        // Only add if it's not already scheduled or if the new update is sooner/higher priority
        if let Some(existing) = self.pending_updates.get(&position) {
            if existing.tick_scheduled < scheduled_tick || 
               (existing.tick_scheduled == scheduled_tick && existing.priority >= priority) {
                return; // Already scheduled for sooner or same time with equal/higher priority
            }
        }
        
        // Add or replace the update
        self.pending_updates.insert(position, update);
    }

    /// Tick the fluid mechanics, processing a limited number of pending updates
    pub async fn tick(&mut self, world: &World, server: &Server) {
        self.current_tick = self.current_tick.wrapping_add(1);
        
        if self.pending_updates.is_empty() {
            return; // Nothing to do
        }
        
        // Find updates ready to process
        let mut updates_to_process = Vec::with_capacity(MAX_UPDATES_PER_TICK);
        
        // Gather updates due this tick
        for update in self.pending_updates.values() {
            if update.tick_scheduled <= self.current_tick {
                updates_to_process.push(update.clone());
                if updates_to_process.len() >= MAX_UPDATES_PER_TICK {
                    break;
                }
            }
        }
        
        if updates_to_process.is_empty() {
            return;
        }
        
        // Sort by priority (higher numbers first)
        updates_to_process.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        // Remove updates we're about to process
        for update in &updates_to_process {
            self.pending_updates.remove(&update.position);
        }
        
        // Clear the batch updates buffer
        self.batch_updates.clear();
        
        // Process each update
        for update in updates_to_process {
            self.process_fluid_update(world, server, &update).await;
        }
        
        // Apply all batched updates
        for (pos, state_id) in &self.batch_updates {
            world.set_block_state(pos, *state_id).await;
        }
    }

    async fn process_fluid_update(&mut self, world: &World, server: &Server, update: &FluidUpdate) {
        let position = update.position;
        
        // Get current state to verify it hasn't changed
        let current_state_id = match world.get_block_state_id(&position).await {
            Ok(id) => id,
            Err(_) => return, // Position is not valid
        };
    
        // Handle air blocks - check for infinite water source first
        if current_state_id == 0 {
            // Count adjacent source blocks
            let mut adjacent_source_count = 0;
            
            // Check for horizontal sources
            for direction in BlockDirection::horizontal() {
                let adjacent_pos = position.offset(direction.to_offset());
                if let Ok(adjacent_id) = world.get_block_state_id(&adjacent_pos).await {
                    if self.is_source_block(adjacent_id) {
                        adjacent_source_count += 1;
                    }
                }
            }
            
            // If there are at least 2 adjacent source blocks, create a new source
            if adjacent_source_count >= 2 {
                // Create a new water source
                self.batch_updates.push((position, WATER_SOURCE_STATE_ID));
                self.schedule_update(position, WATER_SOURCE_STATE_ID, 0, 2);
                return;
            }
            
            self.process_air_block_update(world, &position).await;
            return;
        }
        
        // Skip if state changed
        if current_state_id != update.fluid_state_id {
            return;
        }
        
        // Skip if not water
        if !self.is_water(current_state_id) {
            return;
        }
        
        // Process source blocks
        if self.is_source_block(current_state_id) {
            
            // Always try to flow down first
            let flowed_down = self.try_flow_downward(world, &position).await;
            
            // Then try to flow horizontally
            self.try_flow_source_horizontally(world, &position).await;
            return;
        }
        
        // Process flowing water
        let level = self.get_water_level(current_state_id);
        
        // Check if water has path to source
        if !self.has_source_connection(world, &position).await {
            
            // Reduce level or remove
            if level > 1 {
                // Reduce level by 1
                let new_level = level - 1;
                let new_state_id = self.get_state_id_for_level(new_level);
                
                // Add to batch update
                self.batch_updates.push((position, new_state_id));
                
                // Start orderly receding for connected water blocks
                self.start_water_receding(world, &position, new_level).await;
                
                // Special handling for vertical water columns
                // Check blocks above and below to ensure vertical propagation
                let above_pos = position.offset(BlockDirection::Up.to_offset());
                let below_pos = position.offset(BlockDirection::Down.to_offset());
                
                // Schedule updates for blocks above and below with high priority
                if let Ok(above_state_id) = world.get_block_state_id(&above_pos).await {
                    if self.is_water(above_state_id) && !self.is_source_block(above_state_id) {
                        self.schedule_update(above_pos, above_state_id, 0, 3);
                    }
                }
                
                if let Ok(below_state_id) = world.get_block_state_id(&below_pos).await {
                    if self.is_water(below_state_id) && !self.is_source_block(below_state_id) {
                        self.schedule_update(below_pos, below_state_id, 0, 3);
                    }
                }
            } else {
                // At level 1, remove water
                self.batch_updates.push((position, 0));
                
                // Check neighbors for water that might need to recede
                for direction in BlockDirection::all() { // Changed to all directions
                    let adjacent_pos = position.offset(direction.to_offset());
                    if let Ok(adjacent_state_id) = world.get_block_state_id(&adjacent_pos).await {
                        if self.is_water(adjacent_state_id) && !self.is_source_block(adjacent_state_id) {
                            let adjacent_level = self.get_water_level(adjacent_state_id);
                            
                            // Schedule the adjacent block to check its source connection
                            // with a small delay to ensure orderly processing
                            self.schedule_update(adjacent_pos, adjacent_state_id, 1, 2);
                        }
                    }
                }
            }
            return;
        }
        
        // Always try to flow down first
        let flowed_down = self.try_flow_downward(world, &position).await;
        
        // Always try to flow horizontally
        self.try_flow_horizontally(world, &position, level).await;
        
        // CRITICAL WATERFALL LOGIC
        // Also explicitly check if we can create a waterfall from each side
        if level > 1 { // Only if water level is high enough to flow horizontally
            for direction in BlockDirection::horizontal() {
                let side_pos = position.offset(direction.to_offset());
                let below_side_pos = side_pos.offset(BlockDirection::Down.to_offset());
                
                // Check if side position is air or replaceable (can flow into it)
                let can_flow_side = if let Ok(side_state) = world.get_block_state(&side_pos).await {
                    side_state.air || side_state.replaceable
                } else {
                    false
                };
                
                // Check if below side position is air (waterfall can form)
                let below_is_air = if let Ok(below_state) = world.get_block_state(&below_side_pos).await {
                    below_state.air || below_state.replaceable
                } else {
                    false
                };
                
                // If we can flow sideways and there's air below = potential waterfall
                if can_flow_side && below_is_air {
                    
                    // Place flowing water in the side position
                    let side_level = level - 1;
                    let side_state_id = self.get_state_id_for_level(side_level);
                    
                    // Immediately place the water (don't use batch update system for this critical change)
                    world.set_block_state(&side_pos, side_state_id).await;
                    
                    // Schedule high-priority update to continue the waterfall
                    self.schedule_update(side_pos, side_state_id, 0, 3);
                    self.schedule_update(below_side_pos, 0, 0, 3);
                }
            }
        }
    }
   
    /// Initiates an orderly removal of water when source connection is lost
    async fn start_water_receding(&mut self, world: &World, position: &BlockPos, level: i32) {
        // Get immediate neighbors
        let mut neighbors = Vec::new();
        
        // First find all neighbors that are water with same or lower level
        for direction in BlockDirection::horizontal() {
            let adjacent_pos = position.offset(direction.to_offset());
            
            if let Ok(adjacent_state_id) = world.get_block_state_id(&adjacent_pos).await {
                if self.is_water(adjacent_state_id) && !self.is_source_block(adjacent_state_id) {
                    let adjacent_level = self.get_water_level(adjacent_state_id);
                    
                    // Only consider lower or same level water blocks
                    if adjacent_level <= level {
                        neighbors.push((adjacent_pos, adjacent_level));
                    }
                }
            }
        }
        
        // ADDED: Also check the block above if it's water
        let above_pos = position.offset(BlockDirection::Up.to_offset());
        if let Ok(above_state_id) = world.get_block_state_id(&above_pos).await {
            if self.is_water(above_state_id) && !self.is_source_block(above_state_id) {
                let above_level = self.get_water_level(above_state_id);
                neighbors.push((above_pos, above_level));
            }
        }
        
        // ADDED: Check the block below if it's water
        let below_pos = position.offset(BlockDirection::Down.to_offset());
        if let Ok(below_state_id) = world.get_block_state_id(&below_pos).await {
            if self.is_water(below_state_id) && !self.is_source_block(below_state_id) {
                let below_level = self.get_water_level(below_state_id);
                neighbors.push((below_pos, below_level));
            }
        }
        
        // Sort by level, lowest level first (furthest from source)
        neighbors.sort_by(|a, b| a.1.cmp(&b.1));
        
        // Schedule updates for furthest blocks first with carefully staggered delays
        for (i, (pos, level)) in neighbors.iter().enumerate() {
            // The furthest blocks (lowest level) get updated first
            // Delay increases as we get closer to the source
            // This ensures water recedes from furthest to closest
            let delay = i as u32;
            
            // Schedule as high priority (furthest blocks have highest priority)
            let priority = 3 - i.min(3) as i32; // Convert to priority (3=highest, 0=lowest)
            
            // Schedule the update
            self.schedule_update(*pos, self.get_state_id_for_level(*level), delay, priority);
        }
        
        // Finally, schedule this block for reduction with a delay proportional to its level
        // Higher level blocks (closer to source) get more delay
        // This ensures water recedes in the right order
        let this_delay = neighbors.len() as u32;
        self.schedule_update(*position, self.get_state_id_for_level(level), this_delay, 1);
    }
    
    /// Process an air block to see if it should become water
    async fn process_air_block_update(&mut self, world: &World, position: &BlockPos) {
        // Count adjacent source blocks - for infinite water
        let mut adjacent_source_count = 0;
        
        // Check for horizontal sources first
        for direction in BlockDirection::horizontal() {
            let adjacent_pos = position.offset(direction.to_offset());
            if let Ok(adjacent_id) = world.get_block_state_id(&adjacent_pos).await {
                if self.is_source_block(adjacent_id) {
                    adjacent_source_count += 1;
                }
            }
        }
        
        // If there are at least 2 adjacent source blocks horizontally, create a new source
        if adjacent_source_count >= 2 {
            // Create a new water source
            world.set_block_state(position, WATER_SOURCE_STATE_ID).await;
            self.schedule_update(*position, WATER_SOURCE_STATE_ID, 0, 2);
            return;
        }
        
        // Check for water sources above first (vertical flow)
        let above_pos = position.offset(BlockDirection::Up.to_offset());
        if let Ok(above_id) = world.get_block_state_id(&above_pos).await {
            if self.is_water(above_id) {
                // Water flows downward, place flowing water
                world.set_block_state(position, WATER_LEVEL_7_STATE_ID).await;
                self.schedule_update(*position, WATER_LEVEL_7_STATE_ID, 0, 1);
                return;
            }
        }
        
        // Check for horizontal water flow
        let mut highest_level = 0;
        for direction in BlockDirection::horizontal() {
            let adjacent_pos = position.offset(direction.to_offset());
            if let Ok(adjacent_id) = world.get_block_state_id(&adjacent_pos).await {
                if self.is_source_block(adjacent_id) {
                    // Adjacent to water source, place level 7 water if there's ground below
                    let below_pos = position.offset(BlockDirection::Down.to_offset());
                    if let Ok(below_state) = world.get_block_state(&below_pos).await {
                        if !below_state.air && !below_state.replaceable {
                            world.set_block_state(position, WATER_LEVEL_7_STATE_ID).await;
                            self.schedule_update(*position, WATER_LEVEL_7_STATE_ID, 0, 1);
                            return;
                        }
                    }
                } else if self.is_water(adjacent_id) {
                    // Adjacent to flowing water, track highest level
                    let level = self.get_water_level(adjacent_id);
                    if level > highest_level {
                        highest_level = level;
                    }
                }
            }
        }
        
        // Place flowing water if level is high enough and there's solid ground below
        if highest_level > 1 {
            let new_level = highest_level - 1;
            let below_pos = position.offset(BlockDirection::Down.to_offset());
            
            if let Ok(below_state) = world.get_block_state(&below_pos).await {
                if !below_state.air && !below_state.replaceable {
                    let new_state_id = self.get_state_id_for_level(new_level);
                    world.set_block_state(position, new_state_id).await;
                    self.schedule_update(*position, new_state_id, 0, 1);
                }
            }
        }
    }

    /// Check if a flowing water block has a connection to a source
    async fn has_source_connection(&self, world: &World, position: &BlockPos) -> bool {
        // Get the current state ID
        let current_state_id = match world.get_block_state_id(position).await {
            Ok(id) => id,
            Err(_) => return false,
        };
        
        // If this is a source block already, then yes
        if self.is_source_block(current_state_id) {
            return true;
        }
        
        // Get current water level
        let current_level = self.get_water_level(current_state_id);
        
        // Quick check for adjacent sources - common case optimization
        for direction in BlockDirection::all() {
            let adjacent_pos = position.offset(direction.to_offset());
            if let Ok(adjacent_state_id) = world.get_block_state_id(&adjacent_pos).await {
                if self.is_source_block(adjacent_state_id) {
                    return true;
                }
            }
        }
        
        // If we're at level 7, must be adjacent to a source, which we didn't find
        if current_level == 7 {
            return false;
        }
        
        // DFS to find path to a source by following increasing levels
        let mut visited = HashSet::new();
        visited.insert(*position);
        
        let mut stack = Vec::new();
        
        // Add adjacent water blocks with higher levels to start search
        for direction in BlockDirection::horizontal() {
            let next_pos = position.offset(direction.to_offset());
            if visited.contains(&next_pos) {
                continue;
            }
            
            if let Ok(next_state_id) = world.get_block_state_id(&next_pos).await {
                if self.is_water(next_state_id) {
                    let next_level = self.get_water_level(next_state_id);
                    
                    // Only consider higher level water blocks as potential paths to source
                    if next_level > current_level {
                        stack.push(next_pos);
                        visited.insert(next_pos);
                    }
                }
            }
        }
        
        // Check above block too
        let above_pos = position.offset(BlockDirection::Up.to_offset());
        if let Ok(above_state_id) = world.get_block_state_id(&above_pos).await {
            if self.is_water(above_state_id) {
                stack.push(above_pos);
                visited.insert(above_pos);
            }
        }
        
        // Perform depth-first search
        while let Some(current_pos) = stack.pop() {
            // Check if current position is a source
            if let Ok(state_id) = world.get_block_state_id(&current_pos).await {
                if self.is_source_block(state_id) {
                    return true;
                }
                
                // Get level of current position for comparison
                let pos_level = self.get_water_level(state_id);
                
                // Check all directions
                for direction in BlockDirection::horizontal() {
                    let next_pos = current_pos.offset(direction.to_offset());
                    
                    if visited.contains(&next_pos) {
                        continue;
                    }
                    
                    if let Ok(next_state_id) = world.get_block_state_id(&next_pos).await {
                        // Found a source
                        if self.is_source_block(next_state_id) {
                            return true;
                        }
                        
                        // Only follow path to higher level water
                        if self.is_water(next_state_id) {
                            let next_level = self.get_water_level(next_state_id);
                            
                            if next_level > pos_level {
                                stack.push(next_pos);
                                visited.insert(next_pos);
                            }
                        }
                    }
                }
                
                // Also check above
                let above_next = current_pos.offset(BlockDirection::Up.to_offset());
                if !visited.contains(&above_next) {
                    if let Ok(above_state_id) = world.get_block_state_id(&above_next).await {
                        if self.is_source_block(above_state_id) {
                            return true;
                        }
                        
                        if self.is_water(above_state_id) {
                            stack.push(above_next);
                            visited.insert(above_next);
                        }
                    }
                }
            }
        }
        
        false // No path to source found
    }

    /// Try to flow downward
    async fn try_flow_downward(&mut self, world: &World, position: &BlockPos) -> bool {
        let below_pos = position.offset(BlockDirection::Down.to_offset());
        
        // Check if can flow into position below
        if let Ok(below_id) = world.get_block_state_id(&below_pos).await {
            if below_id == 0 || (self.is_water(below_id) && !self.is_source_block(below_id)) {
                // Can flow down - always place level 7 water below
                let flowing_state_id = WATER_LEVEL_7_STATE_ID;
                
                // Add to batch update
                self.batch_updates.push((below_pos, flowing_state_id));
                
                // Schedule update for the block we just placed - high priority!
                self.schedule_update(below_pos, flowing_state_id, 0, 3);
                
                return true;
            }
        }
        
        false
    }

    /// Try to flow horizontally from a source block
    async fn try_flow_source_horizontally(&mut self, world: &World, position: &BlockPos) {
        // Sources create level 7 water
        let next_level = 7;
        let next_state_id = self.get_state_id_for_level(next_level);
        
        // Calculate flow weights
        let weights = self.calculate_flow_weights(world, position).await;
        
        // Find minimum weight
        let min_weight = weights.iter()
            .map(|(_, weight)| *weight)
            .min()
            .unwrap_or(DEFAULT_FLOW_WEIGHT);
        
        // No valid flow if all weights are max
        if min_weight == DEFAULT_FLOW_WEIGHT {
            return;
        }
        
        // Flow in directions with lowest weight
        for (direction, weight) in &weights {
            if *weight == min_weight {
                let adjacent_pos = position.offset(direction.to_offset());
                
                // Check if position is replaceable
                if let Ok(adjacent_state) = world.get_block_state(&adjacent_pos).await {
                    if adjacent_state.air || adjacent_state.replaceable {
                        // IMPORTANT CHANGE: Check if there's ground below, but don't require it
                        let below_adjacent = adjacent_pos.offset(BlockDirection::Down.to_offset());
                        let has_ground_below = if let Ok(below_state) = world.get_block_state(&below_adjacent).await {
                            !below_state.air && !below_state.replaceable
                        } else {
                            false
                        };
                        
                        // Check if already has better water
                        let can_place = if let Ok(existing_id) = world.get_block_state_id(&adjacent_pos).await {
                            if self.is_water(existing_id) {
                                if self.is_source_block(existing_id) {
                                    false // Don't replace sources
                                } else {
                                    let existing_level = self.get_water_level(existing_id);
                                    existing_level < next_level // Only replace if existing is worse
                                }
                            } else {
                                true // Can replace non-water
                            }
                        } else {
                            false
                        };
                        
                        if can_place {
                            // Add to batch update
                            self.batch_updates.push((adjacent_pos, next_state_id));
                            
                            // Schedule update with higher priority if no ground below to create waterfall
                            let priority = if has_ground_below { 1 } else { 2 };
                            self.schedule_update(adjacent_pos, next_state_id, 0, priority);
                            
                            // Also specifically check block below for waterfalls
                            if !has_ground_below {
                                self.schedule_update(below_adjacent, 0, 0, 2);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Try to flow horizontally from flowing water
    async fn try_flow_horizontally(&mut self, world: &World, position: &BlockPos, current_level: i32) {
        // Skip if at lowest level
        if current_level <= 1 {
            return;
        }
        
        // Horizontal flow decreases level by 1
        let next_level = current_level - 1;
        let next_state_id = self.get_state_id_for_level(next_level);
        
        // For each direction, check and flow
        for direction in BlockDirection::horizontal() {
            let adjacent_pos = position.offset(direction.to_offset());
            
            // Skip if we already have water with better or equal level
            let should_flow = if let Ok(adjacent_state_id) = world.get_block_state_id(&adjacent_pos).await {
                if self.is_source_block(adjacent_state_id) {
                    false // Don't replace sources
                } else if self.is_water(adjacent_state_id) {
                    let existing_level = self.get_water_level(adjacent_state_id);
                    existing_level < next_level // Only flow if existing is worse
                } else {
                    // Not water, check if position is replaceable
                    if let Ok(adjacent_state) = world.get_block_state(&adjacent_pos).await {
                        adjacent_state.air || adjacent_state.replaceable
                    } else {
                        false
                    }
                }
            } else {
                false
            };
            
            if should_flow {
                // Check if there's air below the adjacent position (waterfall opportunity)
                let below_adjacent = adjacent_pos.offset(BlockDirection::Down.to_offset());
                let below_is_air = if let Ok(below_state) = world.get_block_state(&below_adjacent).await {
                    below_state.air || below_state.replaceable
                } else {
                    false
                };
                
                // Place flowing water in adjacent position
                self.batch_updates.push((adjacent_pos, next_state_id));
                
                // Schedule appropriate updates based on whether this is a waterfall opportunity
                if below_is_air {
                    // This is a waterfall opportunity - high priority!
                    self.schedule_update(adjacent_pos, next_state_id, 0, 3);
                    self.schedule_update(below_adjacent, 0, 0, 3);
                } else {
                    // Normal flow
                    self.schedule_update(adjacent_pos, next_state_id, 0, 1);
                }
                
                // Schedule checks for blocks adjacent to this one
                for next_dir in BlockDirection::horizontal() {
                    if next_dir != direction.opposite() { // Don't check where we came from
                        let next_pos = adjacent_pos.offset(next_dir.to_offset());
                        if let Ok(next_id) = world.get_block_state_id(&next_pos).await {
                            if next_id == 0 {
                                self.schedule_update(next_pos, 0, 0, 1);
                            }
                        }
                    }
                }
            }
        }
    }
    
    /// Calculate flow weights for each direction
    async fn calculate_flow_weights(&self, world: &World, position: &BlockPos) -> [(BlockDirection, i32); 4] {
        // Initialize with default weights
        let mut weights = [
            (BlockDirection::North, DEFAULT_FLOW_WEIGHT),
            (BlockDirection::South, DEFAULT_FLOW_WEIGHT),
            (BlockDirection::West, DEFAULT_FLOW_WEIGHT),
            (BlockDirection::East, DEFAULT_FLOW_WEIGHT)
        ];
        
        // For each direction, check if there's a path downward
        for i in 0..weights.len() {
            let direction = weights[i].0;
            let adjacent_pos = position.offset(direction.to_offset());
            
            // Skip if we can't flow here
            if let Ok(adjacent_state) = world.get_block_state(&adjacent_pos).await {
                if !adjacent_state.air && !adjacent_state.replaceable {
                    continue;
                }
                
                // Check if solid block below (immediate downward path)
                let below_adjacent = adjacent_pos.offset(BlockDirection::Down.to_offset());
                if let Ok(below_state) = world.get_block_state(&below_adjacent).await {
                    if !below_state.air && !below_state.replaceable {
                        // Solid block below, weight 0
                        weights[i].1 = 0;
                        continue;
                    }
                }
                
                // Find path downward
                if let Some(distance) = self.find_path_downward(world, &adjacent_pos).await {
                    weights[i].1 = distance;
                }
            }
        }
        
        weights
    }

    /// Find a path downward from the given position
    async fn find_path_downward(&self, world: &World, start_pos: &BlockPos) -> Option<i32> {
        // BFS to find a downward path
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        
        queue.push_back((*start_pos, 0)); // (position, distance)
        visited.insert(*start_pos);
        
        while let Some((pos, distance)) = queue.pop_front() {
            // Stop if too far
            if distance >= MAX_DOWNWARD_PATH_DISTANCE {
                continue;
            }
            
            // Check horizontal directions
            for direction in BlockDirection::horizontal() {
                let next_pos = pos.offset(direction.to_offset());
                
                // Skip if visited
                if visited.contains(&next_pos) {
                    continue;
                }
                
                // Check if position is flowable/replaceable
                if let Ok(next_state) = world.get_block_state(&next_pos).await {
                    if !next_state.air && !next_state.replaceable {
                        continue;
                    }
                    
                    // Check below for empty space
                    let below_pos = next_pos.offset(BlockDirection::Down.to_offset());
                    if let Ok(below_state) = world.get_block_state(&below_pos).await {
                        if below_state.air || below_state.replaceable {
                            return Some(distance + 1);
                        }
                    }
                    
                    // Add to queue
                    queue.push_back((next_pos, distance + 1));
                    visited.insert(next_pos);
                }
            }
        }
        
        None // No path found
    }
}