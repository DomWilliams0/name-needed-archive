# Backlog

An unorganized, unordered list of tasks to eventually get to. Tasks are deleted from here and moved into `active.md`.


## Entity movement
* navigation graph edges for larger step sizes
	* can fall e.g. 2 or 3m
	* cats can jump several voxels
	* humans can jump 1 or 2 voxels
* path finding should take physical size into account
	* small rodents can take small 1x1x1 routes but people cannot
	* area path does not guarantee the full block path will be wide enough
		* consider multiple area path candidates during navigation?
* lazy path evaluation (area at a time)
* path optimisation (line of sight)
	* avoid brushing too close to obstacles too - they're prone to whizzing up tree trunks if they wander close enough
* wandering should choose a close location instead of random in the world
	* wander should not take them up into stupid places like atop chests
		* consider different edge costs for climbing ontop of stupid things, not considered for wandering/walking
* don't always take the most optimal route, change it slightly so everyone doesn't walk around on exactly the same rails
* path invalidation on world change
* walk speed enum scale (wander, dawdle, walk, sprint, etc)
* improve path finding
	* add higher weight difference for inter-area edges
	* very indirect paths within areas, edge costs need adjusting
* tweak arrival threshold for path waypoints, it's a bit jerky
* bug: recalculating a path while already following one causes hiccup as the path starts 1 block behind them
* apply gravity to item entities too, for when block beneath them is mined
* ensure velocity and acceleration is really m/s instead of voxels/s

## UI/input
* graph for fps/tps history
	* measure ticks per second in perf window
* better tile selection
	* selection shows if selected blocks are occluded
	* depth selection
    * show dimensions in-world
    * arbitrary polygon that can be added to/subtracted from with $modifier+drag
    * button to expand outwards/inwards by 1 block
* better entity selection
	* click and drag to select multiple
	* multiple clicks in the same place to iterate through overlapping entities
	* list of society members in UI to click instead
    * when multiple candidates, choose highest z as it's more likely the intention
* interactive terminal to replace/extend dev ui
	* custom log! handler to show warnings and errors
	* in-game OR pop out OR out of process [ncurses] terminal console that persists over restarts
* fast forward toggle
	* update gameloop to allow changing TPS
	* limit gameloop interpolation to 1.0: can be greater if ticks take too long
* resolve an entity to its displayable name including state in the UI layer only
	* e.g. get item name ("Apple (rotten)"), person name ("Steve (unconscious)")
	* expose via helper on NameComponent and replace all the current duplication with "unnamed"
* when entity is in a container, show that instead of "unknown" position
* ui button to skip up/down to next surface
* inventory window (separate from debug window) to show items in person's inventory/container in a nice way
* common widget for an entity's identifier, e.g. show clickable name, mouse over for EX:Y id and useful state, click to select
* add filtering to entity log view i.e. show/hide ai decisions, path finding, item operations, etc
	* retain entity logs after death for a time
* reflection-like api on components to do actions per-component in ui
* fix overlapping in-world display strings e.g. collision resolution
* KindComponent should specify plural for stacked labels
* in addition to currency activity, display the dse/decision that explains it
* bug: can't select treetop blocks?

## Entity behaviour
* ai incentive to choose the same action as last tick
* (sub)activities take an amount of ticks to complete
* be able to block subactivities for a number of ticks, show progress bar up until end tick - work items?
* food/drink vessels and wastage
* define ai in definitions with a collection of "features"/tags/capabilities rather than raw DSEs/behaviours and current "species"
* if only have 2 hands but have a very high priority to carry something, they can jam it somewhere (armpit or somewhere) and carry more capacity at a slow speed/high risk of falling/tripping/dropping
* if new activity fails immediately on the first tick, they stand there stupidly doing nothing until the next system tick - can this schedule an activity update next tick for them?
* revamp hauling to add different methods
	* carrying (add a new TransformChild component)
	* dragging/pushing
* add check for space anywhere in inventory before deciding to go pick something up, instead of going there and failing immediately

### Society-oriented behaviours
* more society level jobs
	* place blocks, destroying what's currently there (DAG for dependencies)
    * specify thickness for wall building
* ai filtering at the job level on high-level requirements before considering all its subtasks
* preserve info about completed society jobs/tasks to show in the ui
* etiquette/morality "meters" that vary based on personality and mood. if low, they take things reserved for work items, take other's food, consider items stored inside other people's personal containers
* specify a limit on job reservations per task, e.g. 2 maximum on sawing a tree trunk
* expand concept of "society" to just be a gathering/group of related entities that emits jobs, e.g. family, guild, village
	* allow multiple societies, e.g. someone has their family and village, a wild animal has its family/herd
* when issuing a society command, remove any that conflict (e.g. multiple hauls for the same thing)

#### Jobs and tools
* tool requirements for a job
	* generic search for components/tags e.g. "edged", "sharpness>50"
* multiple ways for a tool to be considering and used for a job:
	* hauled next to the work location with the materials
	* equip from storage on the way to the work location when all materials are available
	* already held or equipped
* will need a better way of choosing the best person for a job, currently the first person to consider it will reserve it. possibly a voting phase/stage where each candidate volunteers via AI weight system, then the best candidate is chosen from them
* extend material reservation to include the specific materials in transit for a build, to avoid others considering hauling more when it's already on the way
* personality affects the desiribility of jobs - lazy/weak/injured people dont want to do physical work, selfish people dont want to work on society jobs, only things for themselves

##### Building
* search communal society containers for materials
* ui inputs for the player to request builds
	* lua api for building too
* build cancellation by player
	* drop unconsumed materials
* generic build material requirement engine - consumes generic requirements (not definition names), reports original reqs, tracks remaining reqs
* more data oriented
	* dont use definition names for materials, rather generic descriptions so it can be loose
* allow multiple concurrent builders
	* bug: if multiple people are sharing a gather task, they will keep collecting even after the last one is delivered - panics on extra unexpected delivery
* allow smooth changing of material gathering target job without dropping the current haul
* prioritise material gathering for the most complete job, rather than random/round robin
* better handle a build job being set in a non-air position
* overlapping vertical builds need to be clearer, maybe collapse ui elements together


### Item stacks
* automatically downgrade a stack split into 1 to a single item
* allow overflow into stacks bigger than 65k
* implement actual item comparisons for collapsing identical items into a single stack (e.g. 16xarrow)


## World generation
* better biome generation
	* each biome should define its own elevation noise params to add to base elevation
* improve coastlines
	* rough up coastlines via e.g. random subdivisions or erosion simulation
	* dont treat as a fixed width border around continents
	* merge continents that intersect, instead of forcing a coastline through them
* continent blobs should wrap across planet boundaries
* features e.g. caves, mountain ranges
	* accurate-ish rivers, caves
		* varying river width from streams to large uncrossable rivers
		* varying river flow speed
	* magma very low down, or it just gets too hot
	* volcano affects world gen in past
	* trees
		* vary tree height, structure and species
		* grow from saplings into full trees
		* individual branches
		* falling sticks/leaves
		* trees are entities, not (only) blocks
* finite pregenerated world in xy (planet), infinite in z
	* wrapping x,y coordinates is a beefy task, for something that doesnt happen very often
		world loader wraps coords so it never requests slabs out of bounds of the planet
		chunks are loaded and rendered at their true wrapped positions e.g. if worldsize=8, chunks x=0, x=8, x=-8 are the same chunk
		entities must be aware of this! all distance checks must take this into account (https://blog.demofox.org/2017/10/01/calculating-the-distance-between-points-in-wrap-around-toroidal-space/)
		use different base noise for biomes and blend (http://parzivail.com/procedural-terrain-generaion/)
		* chunk and region resolution should wrap around explicitly/fail in generator. should the world loader wrap coords
		* add newtype for unit-agnostic distances between worldpoints/viewpoints in voxels/metres
* unique species and settlements with societies to discover in different environments
	* underground species with no eyes, cave houses
	* underwater people
	* mud crabs with human arms
	* savage cavemen who sneak around in darkness, break bones then drag them back to the cave
* generate new terrain when society member explores, rather than just camera movement. config option!
* bug: a change in the middle of 1 chunk triggers bogus occlusion updates across neighbouring slabs. something to do with occlusion comparison
* grass colour and flora depends on biome/moisture
* different continents could have different variations
* subfeatures can overlap because theyre generated in parallel, e.g. plants and tree trunks. prune collisions across all (sub)features during rasterization? ensure mixing of features is possible, e.g. rivers through forests

### Plants

* more variations with proper characteristics
* better placement in procgen
* better graphics - (procedural) texture?
* growth/death lifecycle of plants
* seeds that can spread and grow new plants
    * grow from corpses/poo
* approximate scattering of nonquantitative growth like grass - no individual entities but rather a distribution of densities. animals can graze on grass at any point and it becomes visibly worn there
    * could be rendered with a shader as it's very dynamic

## Voxel world mechanics
* fluid blocks
	* infinite sources/flows at the world edges
* modification
	* entities building/placing blocks
	* block damage e.g. from explosion
	* side effect of interacting with a block
* varying dimensions
	* e.g. visually tree trunks are not cubic metres. physically they could still be treated like that though
* puddles/spills/splatters on the ground
	* picked up and spread by entities
* blocks that technically solid but possible (and slow) to pass through
	* hedges, bushes
* map chunks to torus and make the world wrap-around
* chests/container are multiblock entities that can be hauled around and stored inside other containers, NOT voxels!

## Optimizations

### Performance
* allocation reuse
	* cap cached alloc size so memory usage doesnt only go upwards
	* raii struct i.e. `fn start(&mut self) -> TempVec;` `TempVec::drop() {self.backing.clear()}`
* pooled allocations
* spatial queries for entities
* path finding worker thread
	* short term path cache for src -> dst: e.g. ai system requests path for calculating cost, then movement reuses that path for following
* periodic and staggered systems
	* preserve determinism if possible
* slice-aware chunk mesh caching
* influence map for density, sound
* remove unneeded Debug impls/cfg_attr them to speed up compilation
* mesh generation on worker thread
* replace all hashmaps with faster non crypto hashes
* replace Arcs/Rcs that don't need weak refs with `triomphe`/`counting_pointer`
* perfect hashing for component name lookup
* terrain finalizer should not propogate to neighbours if single block changes arent on boundary
* investigate invalidating a slab queued for finalization if terrain updates are applied to it, to avoid doing tons of extra work for nothing. some degree of redundant work is ok though, so the terrain never noticably lags behind player updates and catches up suddenly when all changes are applied together
* move finalizer to thread pool and spawn multiple tasks
* unchecked_unwrap
* inventory and physical body lookups/searches could be expensive, cache unchanged
* biggy: consider using separate ECS universes for long and short living entities, if having multiple geneations alive at a time has large memory usage
* experiment with PGO
* consider replacing expensive area link checking (extending into neighbour to check each block) with simple global lookup of (blockpos, direction, length)
* physics system is unnecessarily checking the bounds of every entity every tick - skip this expensive check if stationary and slab hasn't changed
* when submitting slab changes to the worker pool, cancel any other tasks queued for the same slab as they're now outdated
* investigate thousands of occlusion updates for empty all-air slabs
	* completely solid slabs (air, stone, etc) should be treated as a special case
* switch away from `async_trait` when a non boxing impl is available
* `rusttype` has a lot of arc abuse and temporary allocations - port to `ab_glyph`?
* ensure commonly held components like transform are always used in their entirety in systems, if
    not then split into smaller components. e.g. transform pos+rot in rendering

### Memory usage
* CoW terrain slabs
* store sparse block metadata in the containing slab instead of in each block
* LEAK: opengl sub buffers are being leaked, ~3MB per restart

## Crate release
* voxel world
* world update batcher
* iaus ai
* config with watcher
* dynslot in activity status updates

## Rendering
* textures/sprites/animations
* improved terrain colour palette
* very simple oval shadows beneath entities to show height
* bug: occlusion flickers after world changes, updates are probably being queued for too long
* bug: occlusion shadows above a 9 block drop
* bug: occlusion shadows cast by blocks above current viewing slice (like treetops) look very weird

## Building and testing
* fuzzing
* stress test(s)
* code coverage in CI
* tag pre-alpha commits in develop, and generate changelog in release notes
* add tokio tracing feature to help debug deadlocks
* revisit possible miri-compatibility
	* no file IO, no slog logging, no `inventory` ctor collection...
* provide debug logging release builds
* replace all fs access with resource abstraction, to be able to read from packed archive/miri-compatible runner binary with no IO

## Code quality
* track down unwraps/expects/`as` casts and replace with results
* less repetition in chunk/terrain/chunkbuilder/chunkbuilderapply/slicemut
* define rates, scales, units etc in unit crate e.g. metabolism, durabilities
* add more types for procgen region units instead of arbitrary (f64, f64)
* error context chaining would be VERY useful for fatal errors
* consider using `bugsalot` macros to replace .unwrap()/.expect() with logging and make them continuable

## Engine
* explicit namespacing for entity definitions e.g. "core:food_apple"
* detect if debugger is present/breakpoint is hit and pause the gameloop, to avoid the insane catch up after continuing
* separate binary for definition file validation
	* or a single debug binary with different args to do things and print progress (e.g. path finding, world modification) instead of either visually checking in the game or unit tests
* instead of sleeping to wait for world to load, check if panicked every second
* add a bg async task that checks for panics, and aborts runtime - currently panics can randomly cause deadlocks
* restarting should take better care of async thread pool, panics if restart occurs while still loading terrain
* disable planet cache to /tmp for release/non dev builds
* save games
	* specific dir for saved data
	* common API for saving to that dir
	* move existing random file dumps to there, e.g. log file, ui state, worldgen cache
* improve lua scripting API
	* component access
	* voxel world access
	* autorun scripts in a dir on startup
	* port scenarios from rust to scripts
* saving and loading should use its own entity id/referencing (calculated during save) so that on load new entities are spawned starting from generation 1 again. this might be critical for long running games to ensure the monotonic id+generation count doesn't exceed the max
* remove catchup frames between game start and waiting for initial terrain (only noticeable in debug builds for now

## Entity diversity
* animal species
	* dogs, cats, birds, deer
* individual stats for needs
* cats chase mice
* birds/small animals swarm food crumbs left by someone eating

### Dogs
* dogs pick up sticks, move faster than humans, chase cats
* breeds have different characteristics
	* soft mouth vs hard mouth
	* some more likely to pick up items
	* some more likely to accidewntally break things in their mouth
* can be tamed, become part of society?
* can bond to 1 or 2 humans, follow them about, distressed when separated
* if stressed/not fulfilled, chew and damage things
* smell food carried by others, whine and follow them
	* stronger smell emitted if in hand or in the open, less so if in sealed bag/container
* people (who like dogs) see dogs and go over to pet for emotional support
	* can also play fetch with a stick and other riveting games
* dogs play together e.g. tag
* bug: dogs can break blocks if ordered to

### Sheep

* startlement based on senses
    * propagation through herd
* lambs have a bond with parents, stay near and run to them when startled
* herd DSEs specific to the leader? e.g. lead herd to a new location


## Simulation depth
* entity interaction
	* taming, hunting, social
* entity needs
	* drink
	* toilet
	* social
	* sleep
* enemies/hostiles can break blocks
* thieves/desperate people (e.g. dying of hunger) can ignore item/container ownership and steal things
* animals and plants provide various resources aside from meat
	* skin/leather/bladder for waterskins, clothes, armour
	* bones for tools
	* tree bark as a very weak material
	* woven plant materials
	* milk
	* fur
* seasons that affect weather/events depending on biomes
	* savanna dry season
	* trees lose leaves as a reaction to prolonged cold

### Physical wellbeing
* distinct body parts
* wellbeing of individual parts affects stats
* gradual healing and tending
* track injury causes e.g. arrow in leg, fired by X at time Y with weapon Z
* blood flow that can be blocked off
* inventory system should be on top of physical body, which defines number and availability of slots
	* e.g. parts are marked as mounting points, mounting affects the availability of other mounts (backpack (shoulder) vs briefcase (hand) vs ... skin pocket?? (wherever it is))
