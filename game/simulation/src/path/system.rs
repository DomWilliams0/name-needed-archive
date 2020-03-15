use common::*;
use unit::world::BlockPosition;
use world::{WorldPathSlice, CHUNK_SIZE};

use crate::ecs::*;
use crate::path::follow::PathFollowing;
use crate::steer::SteeringComponent;
use crate::TransformComponent;

/// Holds the current path to follow
#[derive(Default)]
pub struct FollowPathComponent {
    path: Option<PathFollowing>,
}

impl FollowPathComponent {
    /// As much of the path that has been calculated so far
    pub fn path(&self) -> Option<WorldPathSlice> {
        self.path.as_ref().map(|p| p.path_remaining())
    }
}

/// System to assign steering behaviour from current path, if any
pub struct PathSteeringSystem;

impl System for PathSteeringSystem {
    fn tick_system(&mut self, data: &mut TickData) {
        let query = <(
            Read<TransformComponent>,
            Write<FollowPathComponent>,
            Write<SteeringComponent>,
        )>::query();
        for (e, (transform, mut path, mut steer)) in query.iter_entities(data.ecs_world) {
            let following = match path.path {
                Some(ref mut path) => path,
                None => return,
            };

            *steer = match following.next_waypoint(&transform.position) {
                // waypoint
                Some((waypoint, false)) => {
                    if following.changed() {
                        trace!("{}: heading towards {:?}", e, waypoint);
                    }
                    SteeringComponent::seek(waypoint.into())
                }

                // last waypoint
                Some((waypoint, true)) => {
                    if following.changed() {
                        trace!("{}: heading towards final waypoint {:?}", e, waypoint);
                    }
                    SteeringComponent::arrive(waypoint.into())
                }

                // path over
                None => {
                    trace!("{}: arrived at destination", e);
                    event_trace(Event::Entity(EntityEvent::NavigationTargetReached(
                        entity_id(e),
                    )));
                    path.path = None;
                    SteeringComponent::default()
                }
            }
        }
    }
}

/// Temporary (!!) system to assign a path. Will be replaced by a proper system (mark my words).
/// Look it even has "Temp" in its name to show I'm serious
pub struct TempPathAssignmentSystem;

impl System for TempPathAssignmentSystem {
    fn tick_system(&mut self, data: &mut TickData) {
        let mut rand = thread_rng();
        let world = data.voxel_world.borrow();

        let query = <(Read<TransformComponent>, Write<FollowPathComponent>)>::query();
        for (e, (transform, mut path)) in query.iter_entities(data.ecs_world) {
            if path.path.is_none() {
                // uh oh, new path needed

                // get random destination with limit on attempts
                const ATTEMPTS: i32 = 10;
                let mut attempts_left = ATTEMPTS;
                let target = loop {
                    let (x, y) = {
                        let random_chunk = world
                            .all_chunks()
                            .choose(&mut rand)
                            .expect("world should have >0 chunks");
                        let x = rand.gen_range(0, CHUNK_SIZE.as_u16());
                        let y = rand.gen_range(0, CHUNK_SIZE.as_u16());
                        let block = BlockPosition::from((x, y, 0));
                        let world_pos = block.to_world_pos(random_chunk.pos());
                        (world_pos.0, world_pos.1)
                    };

                    // find accessible place in world
                    if let target @ Some(_) = world.find_accessible_block_in_column(x, y) {
                        break target;
                    }

                    attempts_left -= 1;
                    if attempts_left < 0 {
                        warn!(
                            "{}: tried and failed {} times to find a random place to path find to",
                            e, ATTEMPTS,
                        );
                        break None;
                    }
                };

                // calculate path and set as target
                let position = transform.position;
                let full_path = target.and_then(|target| world.find_path(position, target));

                match full_path {
                    Some(_) => info!("{}: found path from {:?} to {:?}", e, position, target),
                    None => debug!(
                        "{}: failed to find a path from {:?} to {:?}",
                        e, position, target
                    ),
                }

                path.path = full_path.map(PathFollowing::new);
                if let Some(tgt) = target {
                    event_trace(Event::Entity(EntityEvent::NewNavigationTarget(
                        entity_id(e),
                        tgt.into(),
                    )));
                }
            }
        }
    }
}