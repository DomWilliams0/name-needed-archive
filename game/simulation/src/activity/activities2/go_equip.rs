use crate::activity::activity2::{Activity2, ActivityContext2, ActivityResult};
use crate::ecs::ComponentGetError;
use crate::{ComponentWorld, Entity, TransformComponent};
use async_trait::async_trait;
use common::*;
use unit::world::WorldPoint;
use world::SearchGoal;

#[derive(Debug, Clone)]
pub struct GoEquipActivity2(Entity);

#[derive(Debug, Error)]
pub enum EquipError {
    #[error("Can't get item transform")]
    MissingTransform(#[from] ComponentGetError),
}

pub enum State {
    Going,
    PickingUp,
}

#[async_trait]
impl Activity2 for GoEquipActivity2 {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> ActivityResult {
        // TODO somehow cancel if any destructive event happens to the item

        // go to the item
        ctx.update_status(State::Going);
        let item_pos = self.find_item(&ctx)?;
        ctx.go_to(item_pos, NormalizedFloat::new(0.8), SearchGoal::Arrive)
            .await?;

        // picky uppy
        ctx.update_status(State::PickingUp);
        ctx.pick_up(self.0).await?;

        Ok(())
    }
}

impl GoEquipActivity2 {
    pub fn new(item: Entity) -> Self {
        Self(item)
    }

    fn find_item(&self, ctx: &ActivityContext2) -> Result<WorldPoint, EquipError> {
        let transform = ctx
            .world
            .component::<TransformComponent>(self.0)
            .map_err(EquipError::MissingTransform)?;

        Ok(transform.position)
    }
}

impl Display for GoEquipActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picking up {}", self.0)
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            State::Going => "Going to item",
            State::PickingUp => "Picking up item",
        })
    }
}
