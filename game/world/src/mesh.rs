use color::ColorRgb;
use common::*;

use crate::chunk::slab::{Slab, SLAB_SIZE};
use crate::chunk::Chunk;
use crate::viewer::SliceRange;
use std::fmt::Debug;
use std::mem::MaybeUninit;
use unit::dim::CHUNK_SIZE;
use unit::world::SliceBlock;

// for ease of declaration. /2 for radius as this is based around the center of the block
const X: f32 = unit::scale::BLOCK_DIAMETER / 2.0;

// 0, 1, 2 | 2, 3, 0
const TILE_CORNERS: [(f32, f32); 4] = [(-X, -X), (X, -X), (X, X), (-X, X)];

pub trait BaseVertex: Copy + Debug {
    fn new(pos: (f32, f32), color: ColorRgb) -> Self;
}

pub fn make_simple_render_mesh<V: BaseVertex>(chunk: &Chunk, slice_range: SliceRange) -> Vec<V> {
    let mut vertices = Vec::<V>::new(); // TODO reuse/calculate needed capacity first
    for slice in chunk.slice_range(slice_range) {
        // TODO skip if slice knows it is empty

        for (block_pos, block) in slice.non_air_blocks() {
            let (bx, by) = {
                let SliceBlock(x, y) = block_pos;
                (
                    // +0.5 to render in the center of the block, which is the block mesh's origin
                    f32::from(x) + 0.5,
                    f32::from(y) + 0.5,
                )
            };
            let color = block.block_type().color();

            let mut block_corners = [MaybeUninit::uninit(); TILE_CORNERS.len()];

            for (i, (fx, fy)) in TILE_CORNERS.iter().enumerate() {
                let ao_lightness = f32::from(block.occlusion().corner(i));

                let color = color * ao_lightness;
                block_corners[i] = MaybeUninit::new(V::new(
                    (
                        fx + bx * unit::scale::BLOCK_DIAMETER,
                        fy + by * unit::scale::BLOCK_DIAMETER,
                    ),
                    color,
                ));
            }

            // flip quad if necessary for AO
            if block.occlusion().should_flip() {
                // TODO also rotate texture

                let last = block_corners[3];
                block_corners.copy_within(0..3, 1);
                block_corners[0] = last;
            }

            // convert corners to vertices
            // safety: all corners have been initialized
            let block_vertices = unsafe {
                [
                    // tri 1
                    block_corners[0].assume_init(),
                    block_corners[1].assume_init(),
                    block_corners[2].assume_init(),
                    // tri 2
                    block_corners[2].assume_init(),
                    block_corners[3].assume_init(),
                    block_corners[0].assume_init(),
                ]
            };
            vertices.extend_from_slice(&block_vertices);
        }
    }

    vertices
}

/// Compile time `min`...
const fn min_const(a: usize, b: usize) -> usize {
    [a, b][(a > b) as usize]
}

#[allow(clippy::many_single_char_names)]
/// Based off this[0] and its insane javascript implementation[1]. An attempt was made to make it
/// more idiomatic and less dense but it stops working in subtle ways so I'm leaving it at this :^)
///  - [0] https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/
///  - [1] https://github.com/mikolalysenko/mikolalysenko.github.com/blob/master/MinecraftMeshes/js/greedy.js
pub(crate) fn make_collision_mesh(
    slab: &Slab,
    out_vertices: &mut Vec<f32>,
    out_indices: &mut Vec<u32>,
) {
    let is_solid = |coord: &[i32; 3]| {
        let coord = [coord[0] as i32, coord[1] as i32, coord[2] as i32];
        slab.grid()[&coord].opacity().solid()
    };

    let mut add_vertex = |x: i32, y: i32, z: i32| {
        let old_size = out_vertices.len();
        out_vertices.extend(&[x as f32, y as f32, z as f32]);
        old_size
    };

    // TODO half blocks

    let dims = [CHUNK_SIZE.as_i32(), CHUNK_SIZE.as_i32(), SLAB_SIZE.as_i32()];
    let mut mask = {
        // reuse the same array for each mask, so calculate the min size it needs to be
        const CHUNK_SZ: usize = CHUNK_SIZE.as_usize();
        const SLAB_SZ: usize = SLAB_SIZE.as_usize();
        const FULL_COUNT: usize = CHUNK_SZ * CHUNK_SZ * SLAB_SZ;
        const MIN_DIM: usize = min_const(CHUNK_SZ, SLAB_SZ);
        [false; FULL_COUNT / MIN_DIM]
    };

    for d in 0..3 {
        let u = (d + 1) % 3;
        let v = (d + 2) % 3;

        // unit vector from current direction
        let mut q = [0; 3];
        q[d] = 1;

        // iterate in slices in dimension direction
        let mut x = [0; 3];
        let mut xd = -1i32;
        while xd < dims[d] {
            x[d] = xd;

            // compute mask
            let mut n = 0;
            for xv in 0..dims[v] {
                x[v] = xv;

                for xu in 0..dims[u] {
                    x[u] = xu;
                    let solid_this = if xd >= 0 { is_solid(&x) } else { false };
                    let solid_other = if xd < dims[d] - 1 {
                        is_solid(&[x[0] + q[0], x[1] + q[1], x[2] + q[2]])
                    } else {
                        false
                    };
                    mask[n] = solid_this != solid_other;
                    n += 1;
                }
            }

            x[d] += 1;
            xd += 1;

            // generate mesh
            n = 0;
            for j in 0..dims[v] {
                let mut i = 0;
                while i < dims[u] {
                    if mask[n] {
                        // width
                        let mut w = 1i32;
                        while mask[n + w as usize] && i + w < dims[u] {
                            w += 1;
                        }

                        // height
                        let mut h = 1;
                        let mut done = false;
                        while j + h < dims[v] {
                            for k in 0..w {
                                if !mask[n + k as usize + (h * dims[u]) as usize] {
                                    done = true;
                                    break;
                                }
                            }

                            if done {
                                break;
                            }

                            h += 1;
                        }

                        // create quad
                        {
                            let (b, du, dv) = {
                                let mut quad_pos = x;
                                quad_pos[u] = i;
                                quad_pos[v] = j;

                                let mut quad_width = [0i32; 3];
                                quad_width[u] = w as i32;

                                let mut quad_height = [0i32; 3];
                                quad_height[v] = h;

                                trace!(
                                    "adding quad at {:?} of size {:?}x{:?}",
                                    quad_pos,
                                    quad_width,
                                    quad_height
                                );

                                (quad_pos, quad_width, quad_height)
                            };

                            // add quad vertices
                            let idx = add_vertex(b[0], b[1], b[2]);
                            add_vertex(b[0] + du[0], b[1] + du[1], b[2] + du[2]);
                            add_vertex(
                                b[0] + du[0] + dv[0],
                                b[1] + du[1] + dv[1],
                                b[2] + du[2] + dv[2],
                            );
                            add_vertex(b[0] + dv[0], b[1] + dv[1], b[2] + dv[2]);

                            // add indices
                            let vs = idx as u32 / 3;
                            let indices = [vs, vs + 1, vs + 2, vs + 2, vs + 3, vs];
                            out_indices.extend_from_slice(&indices);
                        }

                        // __partly__ zero mask
                        for l in 0..h {
                            for k in 0..w {
                                mask[n + k as usize + (l * dims[u]) as usize] = false;
                            }
                        }
                        i += w;
                        n += w as usize;
                    } else {
                        i += 1;
                        n += 1;
                    }
                }
            }
        }

        // fully zero mask for next dimension
        mask.iter_mut().for_each(|i| *i = false);
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockType;
    use crate::chunk::slab::Slab;
    use crate::mesh::make_collision_mesh;

    #[test]
    fn greedy_single_block() {
        let slab = {
            let mut slab = Slab::empty(0);
            slab.slice_mut(0).set_block((0, 0), BlockType::Stone);
            slab
        };

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        make_collision_mesh(&slab, &mut vertices, &mut indices);

        assert_eq!(
            vertices.len(),
            6 /* 6 quads */ * 4 /* 4 verts per quad */ * 3 /* x,y,z per vert */
        );
        assert_eq!(
            indices.len(),
            6 /* 6 quads */ * 6 /* 6 indices per quad */
        );
    }

    #[test]
    fn greedy_column() {
        let slab = {
            let mut slab = Slab::empty(0);
            slab.slice_mut(1).set_block((1, 1), BlockType::Stone);
            slab.slice_mut(2).set_block((1, 1), BlockType::Stone);
            slab
        };

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        make_collision_mesh(&slab, &mut vertices, &mut indices);

        // same as single block above
        assert_eq!(vertices.len(), 6 * 4 * 3);
        assert_eq!(indices.len(), 6 * 6);
    }

    #[test]
    fn greedy_plane() {
        let slab = {
            let mut slab = Slab::empty(0);
            slab.slice_mut(0).fill(BlockType::Stone);
            slab.slice_mut(1).set_block((1, 1), BlockType::Grass);
            slab
        };

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        make_collision_mesh(&slab, &mut vertices, &mut indices);
        assert_eq!(vertices.len(), 168); // more of a regression test
        assert_eq!(indices.len(), 84);
    }
}
