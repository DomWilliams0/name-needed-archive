pub use area_navigation::{AreaGraph, AreaGraphSearchContext, AreaNavEdge, AreaPathError};
pub use block_navigation::{BlockGraph, BlockGraphSearchContext, BlockPathError};
use common::*;
pub use cost::EdgeCost;
pub use path::{
    AreaPath, BlockPath, BlockPathNode, NavigationError, SearchGoal, WorldPath, WorldPathNode,
};
use unit::world::{ChunkLocation, SlabIndex};

mod area_navigation;
mod astar;
mod block_navigation;
mod cost;
pub(crate) mod discovery;
mod path;

/// Area index in a slab. 0 is uninitialized, starts at 1
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SlabAreaIndex(pub u16);

/// An area in a chunk
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct ChunkArea {
    pub slab: SlabIndex,
    pub area: SlabAreaIndex,
}

/// An area in the world
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct WorldArea {
    pub chunk: ChunkLocation,
    pub slab: SlabIndex,
    pub area: SlabAreaIndex,
}

impl WorldArea {
    /// Helper for less verbose tests
    #[cfg(test)]
    pub fn new<C: Into<ChunkLocation>>(chunk: C) -> Self {
        Self::new_with_slab(chunk, SlabIndex(0))
    }

    /// Helper for less verbose tests
    #[cfg(test)]
    pub fn new_with_slab<C: Into<ChunkLocation>>(chunk: C, slab: SlabIndex) -> Self {
        Self {
            chunk: chunk.into(),
            slab,
            area: SlabAreaIndex::FIRST,
        }
    }
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
    pub fn into_world_area(self, chunk_pos: ChunkLocation) -> WorldArea {
        WorldArea {
            chunk: chunk_pos,
            slab: self.slab,
            area: self.area,
        }
    }
}

impl From<WorldArea> for ChunkArea {
    fn from(area: WorldArea) -> Self {
        ChunkArea {
            slab: area.slab,
            area: area.area,
        }
    }
}

impl From<WorldArea> for Vector3 {
    fn from(area: WorldArea) -> Self {
        // is this good for estimating node cost?
        Vector3 {
            x: area.chunk.0 as f32,
            y: area.chunk.1 as f32,
            z: area.slab.into(),
        }
    }
}
impl Debug for WorldArea {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "WorldArea(chunk={:?}, slab={:?}, area={:?})",
            self.chunk, self.slab, self.area
        )
    }
}
