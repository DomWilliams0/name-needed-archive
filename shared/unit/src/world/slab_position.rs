use common::derive_more::*;

use crate::world::{
    BlockCoord, BlockPosition, LocalSliceIndex, SlabIndex, SlabLocation, SliceIndex, WorldPosition,
    CHUNK_SIZE,
};

// TODO consider using same generic pattern as SliceIndex for all points and positions
//  e.g. single Position where x/y can be Global/Block, z is Global/Slab/None

/// A block in a slab
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Into, From)]
pub struct SlabPosition(BlockCoord, BlockCoord, LocalSliceIndex);

impl SlabPosition {
    pub fn new(x: BlockCoord, y: BlockCoord, z: LocalSliceIndex) -> Self {
        assert!(x < CHUNK_SIZE.as_block_coord(), "x={} is out of range", x);
        assert!(y < CHUNK_SIZE.as_block_coord(), "y={} is out of range", y);
        // TODO return option instead of asserting
        Self(x, y, z)
    }

    pub fn new_unchecked(x: BlockCoord, y: BlockCoord, z: LocalSliceIndex) -> Self {
        debug_assert!(x < CHUNK_SIZE.as_block_coord(), "x={} is out of range", x);
        debug_assert!(y < CHUNK_SIZE.as_block_coord(), "y={} is out of range", y);
        Self(x, y, z)
    }

    pub fn to_world_position(self, slab: SlabLocation) -> WorldPosition {
        self.to_block_position(slab.slab)
            .to_world_position(slab.chunk)
    }

    pub fn to_block_position(self, slab_index: SlabIndex) -> BlockPosition {
        BlockPosition::new(self.0, self.1, self.2.to_global(slab_index))
    }

    pub const fn x(self) -> BlockCoord {
        self.0
    }
    pub const fn y(self) -> BlockCoord {
        self.1
    }
    pub const fn z(self) -> LocalSliceIndex {
        self.2
    }
}

impl From<(i32, i32, i32)> for SlabPosition {
    fn from(pos: (i32, i32, i32)) -> Self {
        let (x, y, z) = pos;
        Self::new(x as BlockCoord, y as BlockCoord, SliceIndex::new(z))
    }
}

impl From<[i32; 3]> for SlabPosition {
    fn from(pos: [i32; 3]) -> Self {
        let [x, y, z] = pos;
        Self::new(x as BlockCoord, y as BlockCoord, SliceIndex::new(z))
    }
}

impl From<SlabPosition> for [i32; 3] {
    fn from(p: SlabPosition) -> Self {
        let SlabPosition(x, y, z) = p;
        [i32::from(x), i32::from(y), z.slice()]
    }
}

impl From<BlockPosition> for SlabPosition {
    fn from(p: BlockPosition) -> Self {
        Self::new(p.x(), p.y(), p.z().to_local())
    }
}

impl From<WorldPosition> for SlabPosition {
    fn from(p: WorldPosition) -> Self {
        let p = BlockPosition::from(p);
        Self::new(p.x(), p.y(), p.z().to_local())
    }
}
