use common::derive_more::*;

use crate::dim::CHUNK_SIZE;
use crate::world::WorldPosition;
use std::fmt::{Debug, Formatter};
use std::ops::{Add, Sub};

/// A chunk in the world
#[derive(Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Into, From)]
pub struct ChunkPosition(pub i32, pub i32);

impl From<WorldPosition> for ChunkPosition {
    fn from(wp: WorldPosition) -> Self {
        let WorldPosition(x, y, _) = wp;
        ChunkPosition(
            x.div_euclid(CHUNK_SIZE.as_i32()),
            y.div_euclid(CHUNK_SIZE.as_i32()),
        )
    }
}

impl Add<(i32, i32)> for ChunkPosition {
    type Output = Self;

    fn add(self, rhs: (i32, i32)) -> Self::Output {
        Self(self.0 + rhs.0, self.1 + rhs.1)
    }
}
impl Add<(i16, i16)> for ChunkPosition {
    type Output = Self;

    fn add(self, rhs: (i16, i16)) -> Self::Output {
        Self(self.0 + rhs.0 as i32, self.1 + rhs.1 as i32)
    }
}

impl Debug for ChunkPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.0, self.1)
    }
}

impl Sub<Self> for ChunkPosition {
    type Output = Self;

    fn sub(self, rhs: ChunkPosition) -> Self::Output {
        Self(self.0 - rhs.0, self.1 - rhs.1)
    }
}
