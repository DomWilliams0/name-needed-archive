pub use arrayvec::*;
pub use boolinator::Boolinator;
pub use bumpalo;
pub type BumpVec<'a, T> = bumpalo::collections::Vec<'a, T>;
pub type BumpString<'a> = bumpalo::collections::String<'a>;
pub type BumpBox<'a, T> = bumpalo::boxed::Box<'a, T>;

pub use ::tap::*;
pub use cgmath;
pub use cgmath::{
    Angle, EuclideanSpace, InnerSpace, Matrix, MetricSpace, Rotation2, Rotation3, SquareMatrix,
    VectorSpace, Zero,
};
pub use derivative::Derivative;
pub use derive_more;
pub use displaydoc::Display;
pub use dynslot::DynSlot;
pub use float_cmp::ApproxEq;
pub use itertools::*;
pub use num_derive;
pub use num_traits;
pub use ordered_float::{NotNan, OrderedFloat};
pub use parking_lot;
pub use parse_display;
pub use rand::{self, prelude::*};
pub use rstar;
pub use smallvec::{self, *};
pub use thiserror::{self, Error};
pub use tracy_client;

pub use lazy_static::lazy_static;
pub use logging::{
    self, prelude::*, slog_kv_debug, slog_kv_display, slog_value_debug, slog_value_display,
};
#[cfg(feature = "metrics")]
pub use metrics::{self, declare_entity_metric, entity_metric}; // nop macro declared below for disabled feature
pub use newtype::{NormalizedFloat, Proportion};

// common imports that annoyingly get resolved to other pub exports of std/core
// https://github.com/intellij-rust/intellij-rust/issues/5654
pub use std::{
    error::Error,
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    hash::Hash,
    iter::{empty, once},
    marker::PhantomData,
};

pub type BoxedResult<T> = Result<T, Box<dyn Error>>;

pub type F = f32;
pub type Vector3 = cgmath::Vector3<F>;
pub type Vector2 = cgmath::Vector2<F>;
pub type Point3 = cgmath::Point3<F>;
pub type Point2 = cgmath::Point2<F>;
pub type Matrix4 = cgmath::Matrix4<F>;
pub type Quaternion = cgmath::Quaternion<F>;
pub type Basis2 = cgmath::Basis2<F>;
pub type Rad = cgmath::Rad<F>;
pub type Deg = cgmath::Deg<F>;

#[inline]
pub fn rad(f: F) -> Rad {
    cgmath::Rad(f)
}

#[inline]
pub fn deg(f: F) -> Deg {
    cgmath::Deg(f)
}

pub const AXIS_UP: Vector3 = Vector3::new(0.0, 0.0, 1.0);
pub const AXIS_FWD: Vector3 = Vector3::new(0.0, 1.0, 0.0);
pub const AXIS_FWD_2: Vector2 = Vector2::new(0.0, 1.0);

pub fn forward_angle(angle: Rad) -> Vector2 {
    use cgmath::{Basis2, Rotation};
    let rotation = Basis2::from_angle(-angle);
    rotation.rotate_vector(AXIS_FWD_2)
}

pub fn truncate<F: cgmath::BaseFloat, V: cgmath::InnerSpace<Scalar = F>>(vec: V, max: F) -> V {
    if vec.magnitude2() > (max * max) {
        vec.normalize_to(max)
    } else {
        vec
    }
}

pub mod dynslot;
pub mod input;
pub mod newtype;
pub mod random;
pub mod sized_iter;

pub fn seeded_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_entropy(),
    }
}

#[cfg(not(feature = "metrics"))]
#[macro_export]
macro_rules! declare_entity_metric {
    ($($arg:tt)*) => {};
}
