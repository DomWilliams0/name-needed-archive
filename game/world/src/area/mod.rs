pub use area_navigation::AreaGraph;
pub use block_navigation::BlockGraph;
pub use boundary::ChunkBoundary;
use common::Vector3;
pub use cost::EdgeCost;
pub(crate) use path::AreaPath;
pub use path::{WorldPath, WorldPathSlice};

#[cfg(test)]
pub(crate) use path::AreaPathNode;

use crate::chunk::slab::SlabIndex;
use unit::world::ChunkPosition;

mod area_navigation;
mod block_navigation;
mod boundary;
mod cost;
pub(crate) mod discovery;
mod path;

/// Area index in a slab. 0 is uninitialized, starts at 1
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) struct SlabAreaIndex(pub u8);

/// An area in a chunk
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct ChunkArea {
    pub slab: SlabIndex,
    pub area: SlabAreaIndex,
}

/// An area in the world
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) struct WorldArea {
    pub chunk: ChunkPosition,
    pub slab: SlabIndex,
    pub area: SlabAreaIndex,
}

impl SlabAreaIndex {
    pub const UNINITIALIZED: SlabAreaIndex = SlabAreaIndex(0);
    pub const FIRST: SlabAreaIndex = SlabAreaIndex(1);

    pub fn initialized(self) -> bool {
        self.0 != 0
    }

    pub fn increment(&mut self) {
        self.0 += 1;
    }

    pub fn ok(self) -> Option<Self> {
        if self.initialized() {
            Some(self)
        } else {
            None
        }
    }
}

impl ChunkArea {
    pub fn into_world_area(self, chunk_pos: ChunkPosition) -> WorldArea {
        WorldArea {
            chunk: chunk_pos,
            slab: self.slab,
            area: self.area,
        }
    }
}

impl From<WorldArea> for Vector3 {
    fn from(area: WorldArea) -> Self {
        // is this good for estimating node cost?
        Vector3 {
            x: area.chunk.0 as f32,
            y: area.chunk.1 as f32,
            z: area.slab as f32,
        }
    }
}