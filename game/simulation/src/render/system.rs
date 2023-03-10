use std::convert::TryInto;
use std::rc::Rc;

use serde::de::Error;

use color::Color;
use common::*;
use unit::world::BLOCKS_PER_METRE;

use crate::ecs::*;
use crate::input::{SelectedComponent, SelectedEntities, SelectedTiles, SelectionProgress};

use crate::render::renderer::Renderer;
use crate::render::shape::RenderHexColor;
use crate::render::UiElementComponent;
use crate::string::StringCache;
use crate::transform::{PhysicalComponent, TransformRenderDescription};
use crate::{PlayerSociety, Shape2d, SliceRange, TransformComponent};

#[derive(Debug, Clone, Component, EcsComponent)]
#[storage(VecStorage)]
#[name("render")]
pub struct RenderComponent {
    /// Simple 2d shape
    pub shape: Shape2d,

    /// Simple color
    pub color: Color,
}

/// Wrapper for calling generic Renderer in render system
pub struct RenderSystem<'a, R: Renderer> {
    pub renderer: &'a mut R,
    pub slices: SliceRange,
    pub interpolation: f32,
}

impl<'a, R: Renderer> System<'a> for RenderSystem<'a, R> {
    type SystemData = (
        Read<'a, PlayerSociety>,
        Read<'a, SelectedTiles>,
        Read<'a, SelectedEntities>,
        Read<'a, EntitiesRes>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, RenderComponent>,
        ReadStorage<'a, PhysicalComponent>,
        ReadStorage<'a, SelectedComponent>,
        WriteStorage<'a, DisplayComponent>,
        ReadStorage<'a, UiElementComponent>,
        ReadStorage<'a, KindComponent>,
        ReadStorage<'a, NameComponent>,
    );

    fn run(
        &mut self,
        (
            player_soc,
            block_sel,
            entity_sel,
            entities,
            transform,
            render,
            physical,
            selected,
            mut display,
            ui,
            kind,
            name,
        ): Self::SystemData,
    ) {
        // render entities
        for (e, transform, render, physical, display, selected) in (
            &entities,
            &transform,
            &render,
            &physical,
            (&mut display).maybe(),
            (&selected).maybe(),
        )
            .join()
        {
            if !self.slices.contains(transform.slice()) {
                continue;
            }

            let mut transform_desc = TransformRenderDescription::from(transform);

            // interpolate position
            transform_desc.position = {
                let last_pos: Vector3 = transform.last_position.into();
                let curr_pos: Vector3 = transform.position.into();
                let lerped = last_pos.lerp(curr_pos, self.interpolation);
                lerped.try_into().expect("invalid lerp")
            };

            self.renderer.sim_entity(&transform_desc, render, physical);

            if selected.is_some() {
                self.renderer.sim_selected(&transform_desc, physical);
            }

            if let Some(text) = display.and_then(|d| d.render(|| (e, &kind, &name))) {
                // render a bit below the entity
                transform_desc.position.modify_y(|y| {
                    y - (physical.size.xy_max().metres() * 0.5 * BLOCKS_PER_METRE as f32)
                });
                self.renderer.debug_text(transform_desc.position, text);
            }
        }

        // render player's selections
        if let Some(sel) = block_sel.current() {
            let (from, to) = sel.bounds();
            let (color, finished) = match sel.progress() {
                SelectionProgress::InProgress => (Color::rgb(216, 221, 230), false),
                SelectionProgress::Complete => (Color::rgb(252, 253, 255), true),
            };
            self.renderer
                .selection(from.into(), to.into(), color, finished);
        }

        if let Some(range) = entity_sel.drag_in_progress() {
            let (from, to) = range.bounds();
            let color = Color::rgb(216, 221, 230);
            self.renderer.selection(from, to, color, false);
        }

        // render in-game ui elements above entities
        for (e, transform, ui, display, selected) in (
            &entities,
            &transform,
            &ui,
            (&mut display).maybe(),
            selected.maybe(),
        )
            .join()
        {
            // only render elements for the player's society
            match ui.society() {
                Some(soc) if *player_soc != soc => {
                    continue;
                }
                _ => {}
            }

            if !self.slices.contains(transform.slice()) {
                continue;
            }

            // TODO interpolation needed on ui elements?
            let mut transform_desc = TransformRenderDescription::from(transform);

            // move up vertically above all visible entities
            transform_desc
                .position
                .modify_z(|z| z + self.slices.size() as f32);

            self.renderer
                .sim_ui_element(&transform_desc, ui, selected.is_some());

            if let Some(text) = display.and_then(|d| d.render(|| (e, &kind, &name))) {
                self.renderer.debug_text(transform_desc.position, text);
            }
        }
    }
}

impl<V: Value> ComponentTemplate<V> for RenderComponent {
    fn construct(
        values: &mut Map<V>,
        _: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let color: RenderHexColor = values.get("color")?.into_type()?;
        // TODO when shape2d variants are units, ron just gets "Unit" and fails to parse it
        // manually parse for now until simple shapes are replaced
        let shape = match values.get("shape")?.into_string()?.as_str() {
            "Circle" => Shape2d::Circle,
            "Rect" => Shape2d::Rect,
            _ => {
                return Err(ComponentBuildError::Deserialize(ron::Error::custom(
                    format_args!("bad shape {:?}", values.get("shape")?.into_string()?),
                )))
            }
        };

        Ok(Rc::new(Self {
            color: color.into(),
            shape,
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}

register_component_template!("render", RenderComponent);
