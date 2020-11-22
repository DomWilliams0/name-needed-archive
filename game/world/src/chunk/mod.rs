pub use slab::DeepClone;

pub use self::builder::{ChunkBuilder, ChunkDescriptor};
pub(crate) use self::chunk::SlabLoadingStatus;
pub use self::chunk::{Chunk, ChunkId};
pub use self::terrain::{BaseTerrain, BlockDamageResult, OcclusionChunkUpdate};
pub(crate) use self::terrain::{ChunkTerrain, RawChunkTerrain, WhichChunk};

mod builder;

#[allow(clippy::module_inception)]
mod chunk;

mod double_sided_vec;
pub(crate) mod slab;
pub(crate) mod slice;
mod terrain;
