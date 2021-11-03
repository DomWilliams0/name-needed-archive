use unit::world::WorldPosition;

use crate::definitions::loader::Definition;
use crate::definitions::DefinitionNameComponent;
use crate::ecs::*;
use crate::{ComponentWorld, InnerWorldRef, TransformComponent};
use common::*;

pub trait EntityPosition {
    /// (position, optional rotation)
    fn resolve(&self, world: &InnerWorldRef) -> Result<(WorldPosition, Option<Deg>), BuilderError>;
}

#[derive(Debug, Error)]
pub enum BuilderError {
    #[error("No position specified for entity that requires a transform")]
    MissingPosition,

    #[error("Column is inaccessible: {0:?}")]
    InaccessibleColumn((i32, i32)),

    #[error("Position is not walkable: {0}")]
    PositionNotWalkable(WorldPosition),
}

#[must_use = "Use spawn() to create the entity"]
pub struct DefinitionBuilder<'d, W: ComponentWorld> {
    definition: &'d Definition,
    /// Unnecessary when the temporary DefinitionNameComponent is removed
    uid: String,
    world: &'d W,

    position: Option<Box<dyn EntityPosition>>,
}

impl<'d, W: ComponentWorld> DefinitionBuilder<'d, W> {
    pub fn new(definition: &'d Definition, world: &'d W, uid: &str) -> Self {
        Self {
            definition,
            world,
            uid: uid.to_owned(),
            position: None,
        }
    }

    pub fn with_position<P: EntityPosition + 'static>(mut self, pos: P) -> Self {
        // TODO avoid box by resolving here and storing result
        self.position = Some(Box::new(pos));
        self
    }

    pub fn spawn(self) -> Result<Entity, BuilderError> {
        // resolve position if given
        let world_ref = self.world.voxel_world();
        let world = world_ref.borrow();
        let (pos, rot) = match self.position {
            Some(pos) => {
                let (pos, rot) = pos.resolve(&world)?;
                if world.area(pos).ok().is_some() {
                    (Some(pos), rot)
                } else {
                    return Err(BuilderError::PositionNotWalkable(pos));
                }
            }
            None => (None, None),
        };

        let mut builder = self
            .world
            .create_entity()
            .with(DefinitionNameComponent(self.uid));

        for comp in self.definition.components() {
            builder = comp.instantiate(builder);
        }

        let entity = builder.build().into();

        // set position in transform if present
        if let Ok(mut transform) = self.world.component_mut::<TransformComponent>(entity) {
            if let Some(pos) = pos {
                transform.reset_position(pos.centred())
            } else {
                return Err(BuilderError::MissingPosition);
            }

            if let Some(rot) = rot {
                transform.rotate_to(rot.into());
            }
        }

        Ok(entity)
    }
}

impl EntityPosition for WorldPosition {
    fn resolve(&self, _: &InnerWorldRef) -> Result<(WorldPosition, Option<Deg>), BuilderError> {
        Ok((*self, None))
    }
}

impl EntityPosition for (i32, i32) {
    fn resolve(&self, world: &InnerWorldRef) -> Result<(WorldPosition, Option<Deg>), BuilderError> {
        let (x, y) = *self;
        world
            .find_accessible_block_in_column(x, y)
            .ok_or(BuilderError::InaccessibleColumn((x, y)))
            .map(|pos| (pos, None))
    }
}

impl EntityPosition for (WorldPosition, f32) {
    fn resolve(&self, world: &InnerWorldRef) -> Result<(WorldPosition, Option<Deg>), BuilderError> {
        let (pos, _) = EntityPosition::resolve(&self.0, world)?;
        let rot = deg(self.1);
        Ok((pos, Some(rot)))
    }
}
