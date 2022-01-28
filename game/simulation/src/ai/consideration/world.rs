use ai::{Consideration, ConsiderationParameter, Context, Curve};
use unit::world::{WorldPoint, WorldPosition};

use crate::ai::input::BlockTypeMatch;
use crate::ai::{AiContext, AiInput};

// TODO take into account general world/society size? need some scale
pub struct MyProximityToConsideration(pub WorldPoint);

pub struct BlockTypeMatchesConsideration(pub WorldPosition, pub BlockTypeMatch);

impl Consideration<AiContext> for MyProximityToConsideration {
    fn curve(&self) -> Curve {
        Curve::SquareRoot(1.02, -1.02, 1.0)
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::MyDistance2To(self.0)
    }

    fn parameter(&self) -> ConsiderationParameter {
        // TODO take mobility into account, e.g. more injured = prefer closer
        const MAX_DISTANCE: f32 = 50.0;
        ConsiderationParameter::Range {
            min: 0.25,
            max: MAX_DISTANCE.powi(2),
        }
    }
}

impl Consideration<AiContext> for BlockTypeMatchesConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::BlockTypeMatches(self.0, self.1)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Takes raw distance, returns score 0-1
    fn value(dist: f32) -> f32 {
        let c = MyProximityToConsideration(WorldPoint::new_unchecked(0.0, 0.0, 0.0));
        let dist2 = dist * dist;
        let x = c.consider_input(dist2);
        c.curve().evaluate(x).value()
    }

    #[test]
    fn proximity_consideration() {
        let very_far = dbg!(value(60.0));
        let far = dbg!(value(10.0));
        let closer = dbg!(value(4.0));
        let closerrr = dbg!(value(1.5));
        let arrived = dbg!(value(0.1));

        assert!(very_far <= 0.0);
        assert!(far > very_far);
        assert!(closer > far);
        assert!(arrived > closer);
        assert!(closerrr > closer);
        assert!(arrived > closerrr);
        assert!(arrived >= 1.0);
    }
}
