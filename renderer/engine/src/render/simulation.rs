use std::cell::RefCell;
use std::rc::Rc;

use glium::index::PrimitiveType;
use glium::{implement_vertex, Surface};
use glium::{uniform, Frame};
use glium_sdl2::SDL2Facade;

use color::ColorRgb;
use common::Matrix4;
use simulation::{PhysicalComponent, Renderer, TransformComponent};

use crate::render::debug::{DebugShape, DebugShapes};
use crate::render::{draw_params, load_program, DrawParamType};
use unit::view::ViewPoint;

#[derive(Copy, Clone)]
struct EntityVertex {
    v_pos: [f32; 3],
}
implement_vertex!(EntityVertex, v_pos);

impl EntityVertex {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { v_pos: [x, y, z] }
    }
}

#[derive(Copy, Clone, Default)]
struct EntityInstanceAttributes {
    e_pos: [f32; 3],
    e_color: [f32; 3],
    e_model: [[f32; 4]; 4],
}

implement_vertex!(EntityInstanceAttributes, e_pos, e_color, e_model);

pub struct GliumRenderer {
    program: glium::Program,
    entity_instances: Vec<(TransformComponent, PhysicalComponent)>,
    entity_vertex_buf: glium::VertexBuffer<EntityInstanceAttributes>,
    entity_geometry: (glium::VertexBuffer<EntityVertex>, glium::IndexBuffer<u32>),

    // per frame
    // Option because unset until ``init`` is called each frame
    target: Option<Rc<RefCell<<Self as Renderer>::Target>>>,

    // debug
    debug_shapes: DebugShapes,
}

impl GliumRenderer {
    pub fn new(display: &SDL2Facade) -> Self {
        let program = load_program(display, "entity").unwrap();

        // TODO entity count? maybe use "arraylist" vbos with big chunks e.g. 64
        let entity_instances = Vec::with_capacity(64);

        let entity_vertex_buf =
            glium::VertexBuffer::empty_dynamic(display, entity_instances.capacity()).unwrap();

        // 1m cube mesh, to be scaled in per-instance model matrix
        let entity_geometry = {
            // 8 common vertices in a cube
            let vertices = vec![
                EntityVertex::new(-0.5, -0.5, -0.5),
                EntityVertex::new(0.5, -0.5, -0.5),
                EntityVertex::new(0.5, 0.5, -0.5),
                EntityVertex::new(-0.5, 0.5, -0.5),
                EntityVertex::new(-0.5, -0.5, 0.5),
                EntityVertex::new(0.5, -0.5, 0.5),
                EntityVertex::new(0.5, 0.5, 0.5),
                EntityVertex::new(-0.5, 0.5, 0.5),
            ];

            // 6x6 vertex instances
            let indices = vec![
                3, 1, 0, 2, 1, 3, 2, 5, 1, 6, 5, 2, 6, 4, 5, 7, 4, 6, 7, 0, 4, 3, 0, 7, 7, 2, 3, 6,
                2, 7, 0, 5, 4, 1, 5, 0,
            ];

            (
                glium::VertexBuffer::new(display, &vertices).unwrap(),
                glium::IndexBuffer::new(display, PrimitiveType::TrianglesList, &indices).unwrap(),
            )
        };

        Self {
            program,
            entity_instances,
            entity_vertex_buf,
            entity_geometry,
            target: None,
            debug_shapes: DebugShapes::new(display),
        }
    }
}

pub struct FrameTarget {
    pub frame: Frame,
    pub projection: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
}

impl Renderer for GliumRenderer {
    type Target = FrameTarget;

    fn init(&mut self, target: Rc<RefCell<Self::Target>>) {
        self.target = Some(target);
    }

    fn start(&mut self) {
        self.entity_instances.clear();
    }

    fn entity(&mut self, transform: &TransformComponent, physical: &PhysicalComponent) {
        // TODO for safety until it can be expanded
        assert!(self.entity_instances.len() < self.entity_instances.capacity());
        self.entity_instances.push((*transform, *physical));
    }

    fn finish(&mut self) {
        {
            let mut target = self
                .target
                .as_ref()
                .expect("init was not called")
                .borrow_mut();

            // update instance attributes
            {
                let mut mapping = self.entity_vertex_buf.map();
                for (src, dest) in self.entity_instances.iter().zip(mapping.iter_mut()) {
                    // keep attribute position in world coordinates
                    dest.e_pos = src.0.position.into();
                    dest.e_color = src.1.color.into();

                    let (sx, sy, sz) = src.1.dimensions;
                    let model = {
                        let scale = Matrix4::from_nonuniform_scale(sx, sy, sz);
                        let angle = src.0.rotation_angle();
                        let rotation = Matrix4::from_angle_z(angle);
                        rotation * scale // must be in this order
                    };
                    dest.e_model = model.into();
                }
            }

            // render instances
            let uniforms = uniform! {
                proj: target.projection,
                view: target.view,
                instance_count: self.entity_instances.len() as i32,
            };

            let (verts, indices) = &self.entity_geometry;

            target
                .frame
                .draw(
                    (
                        verts,
                        self.entity_vertex_buf
                            .per_instance()
                            .expect("instancing unsupported"),
                    ),
                    indices,
                    &self.program,
                    &uniforms,
                    &draw_params(DrawParamType::Entity),
                )
                .unwrap();
        }
    }

    fn deinit(&mut self) {
        self.target = None;
    }

    fn debug_add_line(&mut self, from: ViewPoint, to: ViewPoint, color: ColorRgb) {
        self.debug_shapes.shapes.push(DebugShape::Line {
            points: [from, to],
            color,
        })
    }

    fn debug_add_tri(&mut self, points: [ViewPoint; 3], color: ColorRgb) {
        self.debug_shapes
            .shapes
            .push(DebugShape::Tri { points, color })
    }

    fn debug_finish(&mut self) {
        let mut target = self
            .target
            .as_ref()
            .expect("init was not called")
            .borrow_mut();

        self.debug_shapes.draw(&mut target);
    }
}