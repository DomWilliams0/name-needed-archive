use common::*;

use unit::world::WorldPoint;

use crate::activity::context::{ActivityContext, DistanceCheckResult, EventResult};
use crate::activity::status::Status;

use crate::ecs::*;
use crate::event::prelude::*;
use crate::item::{
    ContainerError, ContainersError, EndHaulBehaviour, HaulType, HaulableItemComponent,
    HauledItemComponent,
};

use crate::queued_update::QueuedUpdates;
use crate::society::job::SocietyJobHandle;
use crate::{
    ComponentWorld, ContainedInComponent, ContainerComponent, EntityEvent, EntityEventPayload,
    InventoryComponent, PhysicalComponent, TransformComponent,
};

#[derive(Debug, Error, Clone)]
pub enum HaulError {
    #[error("Item destroyed/moved by a destructive event")]
    Interrupted,

    #[error("Hauler has no inventory")]
    NoInventory,

    #[error("Hauler doesn't have enough free hands")]
    NotEnoughFreeHands,

    #[error("Item is not valid, haulable or physical")]
    BadItem,

    #[error("Split item stack doesn't match original stack properties")]
    BadStackSplit,

    #[error("Failed to split item stack: {0}")]
    StackSplit(#[from] ContainersError),

    #[error("Invalid source container entity for haul")]
    BadSourceContainer,

    #[error("Invalid target container entity for haul")]
    BadTargetContainer,

    #[error("Hauler doesn't have a transform")]
    BadHauler,

    #[error("Container operation failed: {0}")]
    Container(#[from] ContainerError),

    #[error("Item is too far away to haul")]
    TooFar,

    #[error("Haul was cancelled")]
    Cancelled,

    #[error("Item is already being hauled by {0}")]
    AlreadyHauled(Entity),

    #[error("Something change while cancelling the haul, maybe it was picked up by someone else")]
    AssumptionsChangedDuringAbort,
}

/// Handles the start (picking up) and finish (putting down), and fixing up components on abort
#[must_use]
pub struct HaulSubactivity<'a> {
    ctx: &'a ActivityContext,
    thing: Entity,
    complete: bool,
    needs_transform: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum HaulSource {
    /// Pick up from current location
    PickUp,

    /// Take an amount of items from an item stack
    PickUpSplitStack(u16),

    /// Take out of a container
    Container(Entity),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum HaulTarget {
    /// Drop on the floor
    Drop(WorldPoint),

    /// Put in a container
    Container(Entity),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum HaulPurpose {
    /// No custom behaviour on success
    JustBecause,

    /// Item should be reserved for a job
    MaterialGathering(SocietyJobHandle),
}

// activity is only used on main thread
unsafe impl Send for HaulPurpose {}
unsafe impl Sync for HaulPurpose {}

// TODO depends on item size
const MAX_DISTANCE: f32 = 4.0;

struct StartHaulingStatus(HaulSource);

struct StopHaulingStatus(Option<HaulTarget>);

impl<'a> HaulSubactivity<'a> {
    /// Checks if close enough to start hauling
    pub async fn start_hauling(
        ctx: &'a ActivityContext,
        thing: Entity,
        source: HaulSource,
    ) -> Result<HaulSubactivity<'a>, HaulError> {
        // check distance
        if matches!(source, HaulSource::PickUp | HaulSource::PickUpSplitStack(_)) {
            match ctx.check_entity_distance(thing, MAX_DISTANCE.powi(2)) {
                DistanceCheckResult::NotAvailable => return Err(HaulError::BadItem),
                DistanceCheckResult::TooFar => return Err(HaulError::TooFar),
                DistanceCheckResult::InRange => {}
            }
        }

        ctx.update_status(StartHaulingStatus(source));

        // get item out of container if necessary

        // create instance now, so on drop/failure we can fix up the transform
        let mut subactivity = HaulSubactivity {
            ctx,
            thing,
            complete: false,
            needs_transform: false,
        };

        if let HaulSource::Container(container) = source {
            subactivity.needs_transform = true;

            queue_container_removal(ctx.world().resource(), thing, container);

            // wait for removal and ensure it succeeded
            ctx.subscribe_to_specific_until(
                thing,
                EntityEventType::ExitedContainer,
                |evt| match evt {
                    EntityEventPayload::ExitedContainer(result) => {
                        if let Ok(exited_container) = result.as_ref() {
                            debug_assert_eq!(*exited_container, container);
                        }

                        Ok(result)
                    }
                    _ => Err(evt),
                },
            )
            .await
            .unwrap_or(Err(HaulError::Cancelled))?;
        }

        // start hauling, which will give the item a transform on success
        queue_haul_pickup(ctx.world(), ctx.entity(), thing, source);

        // wait for event and ensure the haul succeeded
        let mut the_result = None;
        ctx.subscribe_to_many_until(
            thing,
            [EntityEventType::HauledButSplit, EntityEventType::Hauled].into_iter(),
            |evt| match evt {
                EntityEventPayload::Hauled(hauler, result) if hauler == ctx.entity() => {
                    the_result = Some(result);
                    EventResult::Consumed
                }
                EntityEventPayload::HauledButSplit(hauler, result) if hauler == ctx.entity() => {
                    let result = match result {
                        Ok(split_stack) => {
                            subactivity.thing = split_stack;
                            Ok(())
                        }
                        Err(err) => Err(err),
                    };
                    the_result = Some(result);
                    EventResult::Consumed
                }
                _ => EventResult::Unconsumed(evt),
            },
        )
        .await;

        the_result.unwrap_or(Err(HaulError::Cancelled))?;

        // defuse bomb, we have a transform now
        subactivity.needs_transform = false;
        Ok(subactivity)
    }

    /// If target is a location in the world, assumes we have already walked to it.
    /// This is the non-interrupted entrypoint
    pub async fn end_haul(
        &mut self,
        target: HaulTarget,
        purpose: &HaulPurpose,
    ) -> Result<(), HaulError> {
        self.complete = true;
        self.end_haul_impl_sync(Some(target))?;

        if matches!(target, HaulTarget::Container(_)) {
            self.end_haul_impl_async_container(target).await?;
        }

        match purpose {
            HaulPurpose::JustBecause => {}
            HaulPurpose::MaterialGathering(job) => {
                let thing = self.thing;
                let job = *job;
                self.ctx.world().resource::<QueuedUpdates>().queue(
                    "reserve material",
                    move |world| {
                        world.helpers_comps().reserve_material_for_job(thing, job)?;
                        Ok(())
                    },
                );
            }
        }

        Ok(())
    }

    /// First part of end hauling that only supports dropping the item, not container. Call other
    /// async method after this to put in container
    fn end_haul_impl_sync(&self, target: Option<HaulTarget>) -> Result<(), HaulError> {
        let updates = self.ctx.world().resource();

        self.ctx.update_status(StopHaulingStatus(target));

        // fix up components next tick infallibly
        queue_stop_hauling(updates, self.thing, self.ctx.entity(), !self.complete);

        let target = match target {
            Some(tgt) if self.complete => {
                // preserve original
                tgt
            }
            _ => {
                // drop at hauler's feet because we were interrupted
                let hauler_pos = self
                    .ctx
                    .world()
                    .component::<TransformComponent>(self.ctx.entity())
                    .map_err(|_| HaulError::BadHauler)?
                    .position;
                HaulTarget::Drop(hauler_pos)
            }
        };

        // place haulee back into the world
        let item = self.thing;
        match target {
            HaulTarget::Drop(pos) => {
                // drop the item in place
                // TODO add some randomness to drop position
                let needs_transform = self.needs_transform;

                updates.queue("drop hauled item", move |world| {
                    if let Ok(comp) = world.component::<ContainedInComponent>(item) {
                        // our assumptions have changed, someone else has picked this item up, do nothing!
                        return Err(HaulError::AlreadyHauled(comp.entity()).into());
                    }

                    if let Ok(mut transform) = world.component_mut::<TransformComponent>(item) {
                        transform.reset_position(pos);
                    } else if needs_transform {
                        // add transform if missing
                        let _ = world.add_now(item, TransformComponent::new(pos));
                    }

                    Ok(())
                });
            }
            HaulTarget::Container(_) => {
                // do this in async end_haul_impl_async_container
            }
        }

        Ok(())
    }

    #[inline]
    async fn end_haul_impl_async_container(&self, target: HaulTarget) -> Result<(), HaulError> {
        let container_entity = match target {
            HaulTarget::Container(e) => e,
            _ => unreachable!("not a container target"),
        };

        // put the item in the container
        queue_put_into_container(
            self.ctx.world().resource(),
            self.thing,
            self.ctx.entity(),
            container_entity,
        );

        // wait for success
        self.ctx
            .subscribe_to_specific_until(self.thing, EntityEventType::EnteredContainer, |evt| {
                match evt {
                    EntityEventPayload::EnteredContainer(result) => Ok(result),
                    _ => Err(evt),
                }
            })
            .await
            .unwrap_or(Err(HaulError::Interrupted))?;

        // dont bother checking which container it entered, we just queued the right one

        Ok(())
    }
}

impl<'a> Drop for HaulSubactivity<'a> {
    fn drop(&mut self) {
        if !self.complete {
            if let Err(err) = self.end_haul_impl_sync(None) {
                warn!("failed to abort haul subactivity: {}", err);
            }
        }
    }
}

impl Display for HaulTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HaulTarget::Drop(pos) => Display::fmt(pos, f),
            HaulTarget::Container(container) => write!(f, "container {}", container),
        }
    }
}

impl Display for HaulSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HaulSource::PickUp | HaulSource::PickUpSplitStack(_) => write!(f, "current position"),
            HaulSource::Container(container) => write!(f, "container {}", container),
        }
    }
}

impl HaulSource {
    pub fn with_entity(entity: Entity, world: &impl ComponentWorld) -> Option<Self> {
        if let Ok(ContainedInComponent::Container(container)) = world.component(entity).as_deref() {
            Some(Self::Container(*container))
        } else if world.has_component::<TransformComponent>(entity) {
            Some(Self::PickUp)
        } else {
            None
        }
    }

    pub fn location(
        &self,
        world: &impl ComponentWorld,
        item: Entity,
    ) -> Result<WorldPoint, HaulError> {
        match self {
            Self::PickUp | Self::PickUpSplitStack(_) => {
                position_of(world, item).ok_or(HaulError::BadItem)
            }
            Self::Container(container) => {
                position_of(world, *container).ok_or(HaulError::BadSourceContainer)
            }
        }
    }
}

impl HaulTarget {
    pub fn location(&self, world: &impl ComponentWorld) -> Option<WorldPoint> {
        match self {
            HaulTarget::Drop(pos) => Some(*pos),
            HaulTarget::Container(container) => position_of(world, *container),
        }
    }
}

// TODO explicit access side for container, e.g. front of chest
fn position_of(world: &impl ComponentWorld, entity: Entity) -> Option<WorldPoint> {
    world
        .component::<TransformComponent>(entity)
        .ok()
        .map(|transform| transform.position)
}

fn queue_container_removal(updates: &QueuedUpdates, item: Entity, container_entity: Entity) {
    updates.queue("remove item from container", move |world| {
        let do_remove = || -> Result<Entity, HaulError> {
            let mut container = world
                .component_mut::<ContainerComponent>(container_entity)
                .map_err(|_| HaulError::BadSourceContainer)?;

            // remove from container
            container.container.remove(item)?;

            // remove contained component
            world.helpers_comps().remove_from_container(item);

            // the item needs a transform to be added back, leave this until
            // the initialisation of the haul subactivity to avoid jerkiness
            Ok(container_entity)
        };

        let result = do_remove();
        world.post_event(EntityEvent {
            subject: item,
            payload: EntityEventPayload::ExitedContainer(result),
        });

        Ok(())
    });
}

fn queue_haul_pickup(world: &EcsWorld, hauler: Entity, item: Entity, source: HaulSource) {
    world
        .resource::<QueuedUpdates>()
        .queue("haul item", move |world| {
            let do_haul = || -> Result<Option<Entity>, HaulError> {
                // check item is alive and not being hauled first, to ensure .insert() succeeds below
                match world.component::<HauledItemComponent>(item) {
                    Ok(hauled) => return Err(HaulError::AlreadyHauled(hauled.hauler)),
                    Err(ComponentGetError::NoSuchEntity(_)) => return Err(HaulError::BadItem),
                    _ => {}
                }

                // get item properties
                let (extra_hands, mut volume, mut size) = {
                    let haulables = world.read_storage::<HaulableItemComponent>();
                    let physicals = world.read_storage::<PhysicalComponent>();
                    match world.components(item, (&haulables, &physicals)) {
                        Some((haulable, physical)) => {
                            (haulable.extra_hands, physical.volume, physical.size)
                        }
                        None => {
                            warn!("item is not haulable"; "item" => item);
                            return Err(HaulError::BadItem);
                        }
                    }
                };

                debug!(
                    "{entity} wants to haul {item} which needs {extra_hands} extra hands",
                    entity = hauler,
                    item = item,
                    extra_hands = extra_hands
                );

                // ensure hauler has space
                let slots = {
                    let mut inventory = world
                        .component_mut::<InventoryComponent>(hauler)
                        .map_err(|_| HaulError::NoInventory)?;

                    // ensure hauler has enough free hands
                     inventory
                        .get_hauling_slots_unbound(extra_hands)
                        .ok_or(HaulError::NotEnoughFreeHands)?
                };

                // get hauler position if needed
                let hauler_pos = {
                    let transforms = world.read_storage::<TransformComponent>();
                    if item.has(&transforms) {
                        // not needed, item already has a transform
                        None
                    } else {
                        let transform = hauler.get(&transforms).ok_or(HaulError::BadHauler)?;
                        Some(transform.position)
                    }
                };


                // split stack if necessary
                let mut split_stack_entity = None;
                let item = if let HaulSource::PickUpSplitStack(n) = source {
                    let split_stack = world.helpers_containers().split_stack(item, n)?;
                    // TODO remerge stack on failure?

                    // validate
                    let (split_extra_hands, split_volume, split_size) = {
                        let haulables = world.read_storage::<HaulableItemComponent>();
                        let physicals = world.read_storage::<PhysicalComponent>();
                        match world.components(split_stack, (&haulables, &physicals)) {
                            Some((haulable, physical)) => {
                                (haulable.extra_hands, physical.volume, physical.size)
                            }
                            None => {
                                warn!("split stack is not haulable"; "item" => item);
                                return Err(HaulError::BadItem);
                            }
                        }
                    };

                    if split_extra_hands != extra_hands {
                        warn!("split stack needs {split} extra hands, but original stack needed {orig}",
                        split = split_extra_hands, orig = extra_hands);
                        return Err(HaulError::BadStackSplit);
                    }

                    // warn but allow change in physical size
                    if split_size != size {
                        warn!("split stack physical size changed"; "split" => ?split_size, "orig" => ?size);
                    }

                    volume = split_volume;
                    size = split_size;
                    split_stack_entity = Some(split_stack);
                    split_stack
                } else {item};

                // everything has been checked, no more errors past this point

                // fill equip slots
                let mut inventory = world
                    .component_mut::<InventoryComponent>(hauler)
                    .map_err(|_| HaulError::NoInventory)?;
                let mut slots = slots.upgrade(&mut inventory).expect("inventory changed unexpectedly");
                slots.fill(item, volume, size);

                // add components
                world
                    .helpers_comps()
                    .begin_haul(item, hauler, hauler_pos, HaulType::CarryAlone);

                Ok(split_stack_entity)
            };

            let result = do_haul();
            let payload = match (source, result) {
                (HaulSource::PickUpSplitStack(_), Ok(Some(stack)))=>EntityEventPayload::HauledButSplit(hauler, Ok(stack)),
                (HaulSource::PickUpSplitStack(_), Ok(None))=> unreachable!(),
                (HaulSource::PickUpSplitStack(_), Err(err))=>EntityEventPayload::HauledButSplit(hauler, Err(err)),
                (_, result) =>EntityEventPayload::Hauled(hauler, result.map(|_| ())),

            };
            world.post_event(EntityEvent {
                subject: item,
                payload,
            });

            Ok(())
        });
}

fn queue_stop_hauling(updates: &QueuedUpdates, item: Entity, hauler: Entity, interrupted: bool) {
    updates.queue("stop hauling item", move |world| {
        // remove components from item
        let behaviour = world
            .helpers_comps()
            .end_haul(item, hauler, interrupted)
            .ok_or(HaulError::AssumptionsChangedDuringAbort)?;

        let count = match behaviour {
            EndHaulBehaviour::Drop => {
                // free holder's hands
                let mut inventory = world
                    .component_mut::<InventoryComponent>(hauler)
                    .map_err(|_| HaulError::NoInventory)?;

                inventory.remove_item(item)
            }
            EndHaulBehaviour::KeepEquipped => {
                // dont remove from inventory
                0
            }
        };

        debug!(
            "{hauler} stopped hauling {item}, removed from {slots} slots",
            hauler = hauler,
            item = item,
            slots = count;
            "behaviour" => ?behaviour,
        );

        Ok(())
    });
}

fn queue_put_into_container(
    updates: &QueuedUpdates,
    item: Entity,
    hauler: Entity,
    container_entity: Entity,
) {
    updates.queue("put hauled item into container", move |world| {
        let do_put = || -> Result<Entity, HaulError> {
            let item_physical = world
                .component::<PhysicalComponent>(item)
                .map_err(|_| HaulError::BadItem)?;

            let mut container = world
                .component_mut::<ContainerComponent>(container_entity)
                .map_err(|_| HaulError::BadTargetContainer)?;

            container
                .container
                .add_with(item, item_physical.volume, item_physical.size)?;

            // added to container successfully, do component dance
            world
                .helpers_comps()
                .add_to_container(item, ContainedInComponent::Container(container_entity));

            trace!("put item into container"; "item" => item,
                "container" => container_entity, "hauler" => hauler
            );

            Ok(container_entity)
        };

        let payload = do_put();
        world.post_event(EntityEvent {
            subject: item,
            payload: EntityEventPayload::EnteredContainer(payload),
        });

        Ok(())
    });
}

impl Status for StartHaulingStatus {
    fn exertion(&self) -> f32 {
        // TODO depends on weight of item
        2.0
    }
}

impl Display for StartHaulingStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            HaulSource::PickUp => f.write_str("Picking up item"),
            HaulSource::PickUpSplitStack(_) => f.write_str("Picking up items from stack"),
            HaulSource::Container(_) => f.write_str("Taking item out of container"),
        }
    }
}

impl Display for StopHaulingStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            None | Some(HaulTarget::Drop(_)) => f.write_str("Dropping item"),
            Some(HaulTarget::Container(_)) => f.write_str("Putting item into container"),
        }
    }
}

impl Status for StopHaulingStatus {
    fn exertion(&self) -> f32 {
        // TODO depends on weight of item
        2.0
    }
}
