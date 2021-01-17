use crate::continent::ContinentMap;
use crate::rasterize::SlabGrid;
use common::*;

use crate::params::PlanetParams;
use crate::region::{RegionLocation, Regions};
use std::sync::Arc;
use tokio::sync::RwLock;
use unit::world::{BlockPosition, ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPosition};

/// Global (heh) state for a full planet, shared between threads
#[derive(Clone)]
pub struct Planet(Arc<RwLock<PlanetInner>>);

unsafe impl Send for Planet {}
unsafe impl Sync for Planet {}

pub struct PlanetInner {
    pub(crate) params: PlanetParams,
    pub(crate) continents: ContinentMap,
    pub(crate) regions: Regions,

    #[cfg(feature = "climate")]
    climate: Option<crate::climate::Climate>,

    #[cfg(feature = "cache")]
    was_loaded: bool,
}

impl Planet {
    // TODO actual error type
    pub fn new(params: PlanetParams) -> BoxedResult<Planet> {
        debug!("creating planet with params {:?}", params);

        let mut continents = None;

        #[cfg(feature = "cache")]
        {
            if !params.no_cache {
                match crate::cache::try_load(&params) {
                    Ok(None) => info!("no cache found, generating from scratch"),
                    Ok(Some(nice)) => {
                        info!("loaded cached planet from disk");
                        continents = Some(nice);
                    }
                    Err(e) => {
                        error!("failed to load planet from cache: {}", e);
                    }
                }
            }
        }

        #[cfg(feature = "cache")]
        let was_loaded = continents.is_some();
        let continents = continents.unwrap_or_else(|| ContinentMap::new(&params));

        let regions = Regions::new(&params);
        let inner = Arc::new(RwLock::new(PlanetInner {
            params,
            continents,
            regions,

            #[cfg(feature = "climate")]
            climate: None,

            #[cfg(feature = "cache")]
            was_loaded,
        }));

        Ok(Self(inner))
    }

    pub async fn initial_generation(&mut self) {
        let mut planet = self.0.write().await;
        let mut planet_rando = StdRng::seed_from_u64(planet.params.seed());

        // initialize generator unconditionally
        planet.continents.init_generator(&mut planet_rando);

        #[cfg(feature = "cache")]
        {
            if planet.was_loaded {
                debug!("skipping generation for planet loaded from cache");
                return;
            }
        }

        info!("generating planet");
        let params = planet.params.clone();

        // place continents
        let (continents, total_blobs) = planet.continents.generate(&mut planet_rando);
        // TODO reject if continent or land blob count is too low
        info!(
            "placed {count} continents with {blobs} land blobs",
            count = continents,
            blobs = total_blobs
        );

        // rasterize continents onto grid and discover depth i.e. distance from land/sea border,
        // and place initial heightmap
        planet.continents.discover();
        drop(planet);

        #[cfg(feature = "climate")]
        {
            use crate::climate::*;
            use crate::progress::*;

            let planet_ref = self.clone();
            let mut progress = match cfg!(feature = "bin") {
                #[cfg(feature = "bin")]
                true if params.render.create_climate_gif => Box::new(
                    GifProgressTracker::new("/tmp/gifs", params.render.gif_threads)
                        .expect("failed to init gif progress tracker"),
                )
                    as Box<dyn ProgressTracker>,

                _ => Box::new(NopProgressTracker) as Box<dyn ProgressTracker>,
            };

            // downgrade planet reference so it can be read from multiple places
            let planet = self.0.read().await;

            let climate = Climate::simulate(
                &planet.continents,
                &params,
                &mut planet_rando,
                |step, climate| {
                    progress.update(step, planet_ref.clone(), climate);
                },
            );

            progress.fini();

            // upgrade planet lock again
            drop(planet);
            let mut planet = self.0.write().await;
            planet.climate = Some(climate);
        }

        #[cfg(feature = "cache")]
        if !params.no_cache {
            let planet = self.0.read().await;
            if let Err(e) = crate::cache::save(&planet) {
                error!("failed to serialize planet: {}", e);
            }
        }
    }

    pub async fn realize_region(&self, region: RegionLocation) {
        let mut inner = self.0.write().await;
        let height_map = inner.continents.generator();
        inner.regions.get_or_create(region, height_map).await;
    }

    pub fn chunk_bounds(&self) -> (ChunkLocation, ChunkLocation) {
        // TODO could have separate copy of planet params per thread if immutable

        // radius is excluding 0,0
        // TODO radius no longer makes sense
        let radius = 5;
        (
            ChunkLocation(-radius, -radius),
            ChunkLocation(radius, radius),
        )
    }

    /// Generates now and does not cache
    pub async fn generate_slab(&self, slab: SlabLocation) -> SlabGrid {
        let region_loc = RegionLocation::from(slab.chunk);

        let mut inner = self.0.write().await;
        let region = {
            let generator = inner.continents.generator();
            inner.regions.get_or_create(region_loc, generator).await
        };
        let chunk_desc = region.chunk(slab.chunk).description();

        // generate base slab terrain from chunk description
        trace!("generating slab terrain"; slab);
        let mut terrain = SlabGrid::default();
        chunk_desc.apply_to_slab(slab.slab, &mut terrain);

        // TODO rasterize features onto slab

        terrain
    }

    pub async fn find_ground_level(&self, block: WorldPosition) -> GlobalSliceIndex {
        let chunk_loc = ChunkLocation::from(block);
        let region_loc = RegionLocation::from(chunk_loc);

        let mut inner = self.0.write().await;
        let region = {
            let generator = inner.continents.generator();
            inner.regions.get_or_create(region_loc, generator).await
        };

        let chunk_desc = region.chunk(chunk_loc).description();

        let block_pos = BlockPosition::from(block);
        chunk_desc.ground_level(block_pos.into())
    }

    /// Instantiate regions and initialize chunks
    pub async fn prepare_for_chunks(&self, (min, max): (ChunkLocation, ChunkLocation)) {
        let regions = (min.0..=max.0)
            .cartesian_product(min.1..=max.1)
            .map(|(cx, cy)| RegionLocation::from(ChunkLocation(cx, cy)))
            .dedup();

        for region in regions {
            self.realize_region(region).await;
        }
    }

    #[cfg(feature = "bin")]
    pub async fn inner(&self) -> impl std::ops::Deref<Target = PlanetInner> + '_ {
        self.0.read().await
    }
}