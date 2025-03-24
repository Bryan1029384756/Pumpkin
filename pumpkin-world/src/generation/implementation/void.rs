use pumpkin_util::math::vector2::Vector2;

use crate::{
    chunk::{ChunkBlocks, ChunkData},
    generation::{
        Seed, WorldGenerator, generator::GeneratorInit,
    },
};

pub struct VoidGenerator;

impl GeneratorInit for VoidGenerator {
    fn new(_seed: Seed) -> Self {
        Self {}
    }
}

impl WorldGenerator for VoidGenerator {
    fn generate_chunk(&self, at: Vector2<i32>) -> ChunkData {
        ChunkData {
            blocks: ChunkBlocks::Homogeneous(0), // Air blocks only
            heightmap: Default::default(),
            position: at,
            dirty: true,
        }
    }
}