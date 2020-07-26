use color::ColorRgb;

use crate::navigation::{ChunkArea, SlabAreaIndex};
use crate::occlusion::BlockOcclusion;
use common::derive_more::Display;
use common::Proportion;
pub use enum_iterator::IntoEnumIterator;
use unit::world::GlobalSliceIndex;

/// A single block in a chunk
// TODO store sparse block data in the slab instead of inline in the block
#[derive(Debug, Copy, Clone)]
pub struct Block {
    block_type: BlockType,

    /// How damaged the block is
    durability: Proportion<BlockDurability>,

    /// Navigability
    area: SlabAreaIndex,
    /// Lighting
    occlusion: BlockOcclusion,
}

pub type BlockDurability = u8;

/// The type of a block
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, IntoEnumIterator, Display)]
pub enum BlockType {
    Air,
    Dirt,
    Grass,
    #[display(fmt = "Light grass")]
    LightGrass,
    Stone,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BlockOpacity {
    Transparent,
    Solid,
}

impl BlockOpacity {
    pub fn solid(self) -> bool {
        if let Self::Solid = self {
            true
        } else {
            false
        }
    }

    pub fn transparent(self) -> bool {
        !self.solid()
    }
}

impl Block {
    fn with_block_type(block_type: BlockType) -> Self {
        Self {
            block_type,
            durability: block_type.durability(),
            area: SlabAreaIndex::UNINITIALIZED,
            occlusion: BlockOcclusion::default(),
        }
    }

    pub const fn air() -> Self {
        Self {
            block_type: BlockType::Air,
            durability: Proportion::default_empty(),
            area: SlabAreaIndex::UNINITIALIZED,
            occlusion: BlockOcclusion::default_const(),
        }
    }

    pub const fn block_type(self) -> BlockType {
        self.block_type
    }

    pub fn block_type_mut(&mut self) -> &mut BlockType {
        &mut self.block_type
    }

    pub fn opacity(self) -> BlockOpacity {
        self.block_type.opacity()
    }

    pub fn walkable(self) -> bool {
        self.area.initialized()
    }

    pub fn walkable_area(self) -> Option<SlabAreaIndex> {
        if self.area.initialized() {
            Some(self.area)
        } else {
            None
        }
    }

    pub(crate) fn area_index(self) -> SlabAreaIndex {
        // TODO this should return an Option if area is uninitialized
        self.area
    }
    pub(crate) fn area_mut(&mut self) -> &mut SlabAreaIndex {
        &mut self.area
    }
    pub(crate) fn chunk_area(self, slice: GlobalSliceIndex) -> Option<ChunkArea> {
        if self.area.initialized() {
            Some(ChunkArea {
                slab: slice.slab_index(),
                area: self.area,
            })
        } else {
            None
        }
    }

    pub fn occlusion_mut(&mut self) -> &mut BlockOcclusion {
        &mut self.occlusion
    }
    pub fn occlusion(&self) -> &BlockOcclusion {
        &self.occlusion
    }

    pub(crate) fn durability_mut(&mut self) -> &mut Proportion<BlockDurability> {
        &mut self.durability
    }
}

impl Default for Block {
    fn default() -> Self {
        Self::with_block_type(BlockType::Air)
    }
}

impl BlockType {
    pub fn color(self) -> ColorRgb {
        match self {
            BlockType::Air => ColorRgb::new(0, 0, 0),
            BlockType::Dirt => ColorRgb::new(86, 38, 23),
            BlockType::Grass => ColorRgb::new(49, 152, 56),
            BlockType::LightGrass => ColorRgb::new(91, 152, 51),
            BlockType::Stone => ColorRgb::new(106, 106, 117),
        }
    }

    pub fn opacity(self) -> BlockOpacity {
        if let BlockType::Air = self {
            BlockOpacity::Transparent
        } else {
            BlockOpacity::Solid
        }
    }

    fn durability(self) -> Proportion<BlockDurability> {
        let max = match self {
            BlockType::Air => 0,
            BlockType::Dirt | BlockType::Grass | BlockType::LightGrass => 40,
            BlockType::Stone => 90,
        };

        Proportion::with_value(max, max)
    }
}

/// Helper
impl Into<Block> for BlockType {
    fn into(self) -> Block {
        Block::with_block_type(self)
    }
}
