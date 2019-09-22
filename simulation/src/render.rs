use std::cell::RefCell;
use std::rc::Rc;

use specs::prelude::*;
use specs_derive::Component;

use world::{SliceRange, World};

use crate::movement::Position;

/// Physical attributes to be rendered
#[derive(Component, Debug, Copy, Clone)]
#[storage(VecStorage)]
pub struct Physical {
    /// temporary flat color
    pub color: (u8, u8, u8),
}

pub trait Renderer {
    type Target;

    fn init(&mut self, _target: Rc<RefCell<Self::Target>>) {}

    fn start(&mut self) {}

    fn entity(&mut self, pos: &Position, physical: &Physical);

    fn finish(&mut self) {}

    // ---

    fn debug_start(&mut self) {}

    fn debug_add_line(&mut self, from: Position, to: Position, color: (u8, u8, u8));

    fn debug_finish(&mut self) {}
}

//#[derive(Clone)]
pub struct FrameRenderState<R: Renderer> {
    pub target: Rc<RefCell<R::Target>>,
    pub slices: SliceRange,
}

impl<R: Renderer> Clone for FrameRenderState<R> {
    fn clone(&self) -> Self {
        Self {
            target: self.target.clone(),
            slices: self.slices,
        }
    }
}

/// Wrapper for calling generic Renderer in render system
pub(crate) struct RenderSystem<'a, R: Renderer> {
    pub renderer: &'a mut R,
    pub frame_state: FrameRenderState<R>,
}

impl<'a, R: Renderer> System<'a> for RenderSystem<'a, R> {
    type SystemData = (ReadStorage<'a, Position>, ReadStorage<'a, Physical>);

    fn run(&mut self, (pos, physical): Self::SystemData) {
        for (pos, physical) in (&pos, &physical).join() {
            if self.frame_state.slices.contains(pos.z) {
                self.renderer.entity(pos, physical);
            }
        }
    }
}

pub trait DebugRenderer<R: Renderer> {
    fn render(
        &mut self,
        renderer: &mut R,
        world: Rc<RefCell<World>>,
        frame_state: &FrameRenderState<R>,
    );
}

/*

/// draws some lines
pub struct DummyDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for DummyDebugRenderer {
    fn render(
        &mut self,
        renderer: &mut R,
        world: Rc<RefCell<World>>,
        _frame_state: &FrameRenderState<R>,
    ) {
        renderer.debug_add_line(
            Position::new(0.0, 0.0, 0),
            Position::new(5.0, 5.0, 5),
            (255, 0, 0),
        );

        renderer.debug_add_line(
            Position::new(0.0, 0.0, 0),
            Position::new(0.0, 20.0, 0),
            (10, 255, 0),
        );

        // outline blocks in slice 2
        for c in world.borrow().visible_chunks() {
            for (pos, _) in c.slice(2).non_air_blocks() {
                let color = (10, 190, 30);
                let x = pos.0 as f32;
                let y = pos.1 as f32;
                let z = 5;

                renderer.debug_add_line(
                    Position::new(x, y, z),
                    Position::new(x + 1.0, y, z),
                    color
                );
                renderer.debug_add_line(
                    Position::new(x + 1.0, y, z),
                    Position::new(x + 1.0, y + 1.0, z),
                    color
                );
                renderer.debug_add_line(
                    Position::new(x + 1.0, y + 1.0, z),
                    Position::new(x, y + 1.0, z),
                    color
                );
            }

        }
    }
}

*/
