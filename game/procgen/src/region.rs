use crate::continent::ContinentMap;
use crate::rasterize::BlockType;
use crate::{PlanetParams, SlabGrid};
use common::*;

use grid::{grid_declare, GridImpl};

use std::mem::MaybeUninit;

use unit::dim::SmallUnsignedConstant;
use unit::world::{
    ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex, SliceBlock, SliceIndex,
    CHUNK_SIZE, SLAB_SIZE,
};

/// Is only valid between 0 and planet size, it's the responsibility of the world loader to only
/// request slabs in valid regions
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RegionLocation(pub u32, pub u32);

/// Each region is broken up into this many chunks per side, i.e. this^2 for total number of chunks
pub const CHUNKS_PER_REGION_SIDE: SmallUnsignedConstant = SmallUnsignedConstant::new(8);

pub const CHUNKS_PER_REGION: usize =
    CHUNKS_PER_REGION_SIDE.as_usize() * CHUNKS_PER_REGION_SIDE.as_usize();

pub struct Regions {
    params: PlanetParams,
    regions: Vec<(RegionLocation, Region)>,
}

/// Each pixel in the continent map is a region. Each region is a 2d grid of chunks.
///
/// Large scale features are generated globally (forest placement, rivers, ore distributions, cave
/// placement, etc) but only stored until a slab is requested. When a range of slabs is
/// requested, initialize all chunks in the region and apply features to slabs in the vertical range.
///
/// Chunk initialization:
///     * Calculate description from block distribution based on position. This is only a
///       description and is not yet rasterized into blocks. e.g.
///        * all air if above ground
///        * surface blocks (grass, stone etc) if at ground level based on heightmap
///        * solid stone underground
///
/// For every large feature that overlaps with this region (in all
/// axes including z, so all underground caves aren't calculated now if only the surface is being
/// generated):
///     * Generate subfeatures if relevant and not already done (tree placement in forest bounds,
///       river curve rasterization into blocks, etc)
///     * Attempt to place all blocks in each subfeature in this region and slab range only
///         * The first time a slab is touched, use chunk description to rasterize initial blocks
pub struct Region {
    chunks: [RegionChunk; CHUNKS_PER_REGION],
}

pub struct RegionChunk {
    desc: ChunkDescription,
}

pub struct ChunkDescription {
    ground_height: ChunkHeightMap,
    /// Incredibly temporary flag to identify "underwater". Exceedingly temporary!!
    is_region_land: bool,
}

grid_declare!(struct ChunkHeightMap<ChunkHeightMapImpl, i32>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    1
);

impl Regions {
    pub fn new(params: &PlanetParams) -> Self {
        Regions {
            params: params.clone(),
            regions: Vec::with_capacity(64),
        }
    }

    pub async fn get_or_create(
        &mut self,
        location: RegionLocation,
        continents: &ContinentMap,
    ) -> Option<&Region> {
        Some(match self.region_index(location)? {
            Ok(idx) => &self.regions[idx].1,
            Err(idx) => {
                debug!("creating new region"; "region" => ?location);
                let region = Region::create(location, continents, &self.params).await;
                self.regions.insert(idx, (location, region));
                &self.regions[idx].1
            }
        })
    }

    pub fn get_existing(&self, region: RegionLocation) -> Option<&Region> {
        self.region_index(region)
            .and_then(|idx| idx.ok())
            .map(|idx| &self.regions[idx].1)
    }

    /// None if out of range of the planet, otherwise Ok(idx) if present or Err(idx) if in range but
    /// not present
    fn region_index(&self, region: RegionLocation) -> Option<Result<usize, usize>> {
        self.params
            .is_region_in_range(region)
            .as_some_from(|| self.regions.binary_search_by_key(&region, |(pos, _)| *pos))
    }
}

impl Region {
    async fn create(
        region: RegionLocation,
        continents: &ContinentMap,
        params: &PlanetParams,
    ) -> Self {
        // using a log_scope here causes a nested panic, possibly due to dropping the scope multiple
        // times?
        debug!("creating region"; "region" => ?region);

        let height_scale = params.height_scale as f64;

        // initialize chunk descriptions
        let mut chunks: [MaybeUninit<RegionChunk>; CHUNKS_PER_REGION] =
            unsafe { MaybeUninit::uninit().assume_init() };

        let continents: &'static ContinentMap = unsafe { std::mem::transmute(continents) };

        let handle = tokio::runtime::Handle::current();
        let results = futures::future::join_all((0..CHUNKS_PER_REGION).map(|idx| {
            // cant pass a ptr across threads but you can an integer :^)
            // the array is stack allocated and we dont leave this function while this closure is
            // alive so this pointer is safe to use.
            let this_chunk = chunks[idx].as_mut_ptr() as usize;
            handle.spawn(async move {
                let chunk = RegionChunk::new(idx, region, continents, height_scale);

                // safety: each task has a single index in the chunk array
                unsafe {
                    let this_chunk = this_chunk as *mut RegionChunk;
                    this_chunk.write(chunk);
                }
            })
        }))
        .await;

        for result in results {
            if let Err(err) = result {
                panic!("panic occurred in future: {}", err);
            }
        }

        // safety: all chunks have been initialized and any panics have been propagated
        let chunks: [RegionChunk; CHUNKS_PER_REGION] = unsafe { std::mem::transmute(chunks) };

        Region { chunks }
    }

    fn chunk_index(chunk: ChunkLocation) -> usize {
        let ChunkLocation(x, y) = chunk;
        let x = x.rem_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        let y = y.rem_euclid(CHUNKS_PER_REGION_SIDE.as_i32());

        (x + (y * CHUNKS_PER_REGION_SIDE.as_i32())) as usize
    }

    pub fn chunk(&self, chunk: ChunkLocation) -> &RegionChunk {
        let idx = Self::chunk_index(chunk);
        debug_assert!(idx < self.chunks.len(), "bad idx {}", idx);
        &self.chunks[idx]
    }
}

impl RegionChunk {
    fn new(
        chunk_idx: usize,
        region: RegionLocation,
        continents: &ContinentMap,
        height_scale: f64,
    ) -> Self {
        const PER_BLOCK: f64 = 1.0 / (CHUNKS_PER_REGION_SIDE.as_f64() * CHUNK_SIZE.as_f64());

        let (rx, ry) = (region.0 as f64, region.1 as f64);

        let chunk_idx = chunk_idx as i32;
        let cx = chunk_idx % CHUNKS_PER_REGION_SIDE.as_i32();
        let cy = chunk_idx / CHUNKS_PER_REGION_SIDE.as_i32();
        let continent_tile = continents.tile_at(region);
        let generator = continents.generator();

        // get height for each surface block in chunk
        let mut height_map = ChunkHeightMap::default();
        let (mut min_height, mut max_height) = (i32::MAX, i32::MIN);
        for (i, (by, bx)) in (0..CHUNK_SIZE.as_i32())
            .cartesian_product(0..CHUNK_SIZE.as_i32())
            .enumerate()
        {
            let nx = rx + (((cx * CHUNK_SIZE.as_i32()) + bx) as f64 * PER_BLOCK);
            let ny = ry + (((cy * CHUNK_SIZE.as_i32()) + by) as f64 * PER_BLOCK);
            let height = generator.sample_normalized((nx, ny));

            // convert height map float into block coords
            // TODO should height scale be per biome?
            let block_height = (height * height_scale) as i32;

            height_map[i] = block_height;

            min_height = min_height.min(block_height);
            max_height = max_height.max(block_height);
        }

        // TODO depends on many local parameters e.g. biome, humidity

        trace!("generated region chunk"; "chunk" => ?(cx, cy), "region" => ?(rx, ry));

        RegionChunk {
            desc: ChunkDescription {
                ground_height: height_map,
                is_region_land: continent_tile.is_land(),
            },
        }
    }

    pub fn description(&self) -> &ChunkDescription {
        &self.desc
    }
}

impl ChunkDescription {
    pub fn apply_to_slab(&self, slab_idx: SlabIndex, slab: &mut SlabGrid) {
        let from_slice = slab_idx.as_i32() * SLAB_SIZE.as_i32();
        let to_slice = from_slice + SLAB_SIZE.as_i32();

        // TODO could do this multiple slices at a time
        for (z_global, z_local) in (from_slice..to_slice)
            .map(GlobalSliceIndex::new)
            .zip(LocalSliceIndex::range())
        {
            let slice = {
                let (from, to) = slab.slice_range(z_local.slice_unsigned());
                &mut slab.array_mut()[from..to]
            };

            // TODO these constants depend on biome, location etc
            let surface_block = BlockType::Grass;
            let shallow_under_block = BlockType::Dirt;
            let deep_under_block = BlockType::Stone;
            let shallow_depth = 3;

            for (i, (y, x)) in (0..CHUNK_SIZE.as_i32())
                .cartesian_product(0..CHUNK_SIZE.as_i32())
                .enumerate()
            {
                let ground = SliceIndex::new(self.ground_height[&[x, y, 0]]);
                let bt = match (ground - z_global).slice() {
                    d if d >= 0 && !self.is_region_land => BlockType::Stone, // temporary, underwater = stone
                    0 => surface_block,
                    d if d.is_negative() => BlockType::Air,
                    d if d < shallow_depth => shallow_under_block,
                    _ => deep_under_block,
                };

                slice[i].ty = bt;
            }
        }
    }

    pub fn ground_level(&self, block: SliceBlock) -> GlobalSliceIndex {
        let SliceBlock(x, y) = block;
        GlobalSliceIndex::new(self.ground_height[&[x as i32, y as i32, 0]])
    }
}

impl RegionLocation {
    /// None if negative
    pub fn try_from_chunk(chunk: ChunkLocation) -> Option<Self> {
        let x = chunk.0.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        let y = chunk.1.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());

        if x >= 0 && y >= 0 {
            Some(RegionLocation(x as u32, y as u32))
        } else {
            None
        }
    }

    /// None if negative or greater than planet size
    pub fn try_from_chunk_with_params(chunk: ChunkLocation, params: &PlanetParams) -> Option<Self> {
        let x = chunk.0.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        let y = chunk.1.div_euclid(CHUNKS_PER_REGION_SIDE.as_i32());
        let limit = 0..params.planet_size as i32;

        if limit.contains(&x) && limit.contains(&y) {
            Some(RegionLocation(x as u32, y as u32))
        } else {
            None
        }
    }

    /// Inclusive bounds
    pub fn chunk_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        let x = self.0 as i32;
        let y = self.1 as i32;

        let min = (
            x * CHUNKS_PER_REGION_SIDE.as_i32(),
            y * CHUNKS_PER_REGION_SIDE.as_i32(),
        );
        let max = (
            min.0 + CHUNKS_PER_REGION_SIDE.as_i32() - 1,
            min.1 + CHUNKS_PER_REGION_SIDE.as_i32() - 1,
        );
        (min.into(), max.into())
    }

    pub fn local_chunk_to_global(&self, local_chunk: ChunkLocation) -> ChunkLocation {
        assert!((0..CHUNKS_PER_REGION_SIDE.as_i32()).contains(&local_chunk.x()));
        assert!((0..CHUNKS_PER_REGION_SIDE.as_i32()).contains(&local_chunk.y()));

        ChunkLocation(
            (self.0 as i32 * CHUNKS_PER_REGION_SIDE.as_i32()) + local_chunk.x(),
            (self.1 as i32 * CHUNKS_PER_REGION_SIDE.as_i32()) + local_chunk.y(),
        )
    }
}

slog_value_debug!(RegionLocation);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_to_region() {
        // negative is always out of range
        assert_eq!(RegionLocation::try_from_chunk(ChunkLocation(-2, 1)), None);

        assert_eq!(
            RegionLocation::try_from_chunk(ChunkLocation(
                CHUNKS_PER_REGION_SIDE.as_i32() / 2,
                CHUNKS_PER_REGION_SIDE.as_i32()
            )),
            Some(RegionLocation(0, 1))
        );
    }

    #[test]
    fn chunk_index() {
        assert_eq!(
            Region::chunk_index(ChunkLocation(0, 2)),
            CHUNKS_PER_REGION_SIDE.as_usize() * 2
        );

        assert_eq!(Region::chunk_index(ChunkLocation(3, 0)), 3);

        assert_eq!(
            Region::chunk_index(ChunkLocation(3 + (CHUNKS_PER_REGION_SIDE.as_i32() * 3), 0)),
            3
        );

        let idx = Region::chunk_index(ChunkLocation(
            CHUNKS_PER_REGION_SIDE.as_i32() - 1,
            CHUNKS_PER_REGION_SIDE.as_i32() - 2,
        ));
        assert_eq!(idx, 55);
        assert_eq!(Region::chunk_index(ChunkLocation(-1, -2)), idx);
    }

    #[tokio::test]
    async fn get_existing_region() {
        let params = {
            let mut params = PlanetParams::dummy();
            params.planet_size = 100;
            params
        };
        let mut regions = Regions::new(&params);
        let mut continents = ContinentMap::new_with_rng(&params, &mut thread_rng());

        let loc = RegionLocation(10, 20);
        let bad_loc = RegionLocation(10, 200);

        assert!(regions.get_existing(loc).is_none());
        assert!(regions.get_existing(bad_loc).is_none());

        assert!(params.is_region_in_range(loc));
        assert!(!params.is_region_in_range(bad_loc));

        assert!(regions.get_or_create(loc, &continents).await.is_some());
        assert!(regions.get_or_create(bad_loc, &continents).await.is_none());

        assert!(regions.get_existing(loc).is_some());
        assert!(regions.get_existing(bad_loc).is_none());
    }
}
