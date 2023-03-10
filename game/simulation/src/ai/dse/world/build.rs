use crate::ai::consideration::MyProximityToConsideration;

use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};

use crate::job::{BuildDetails, SocietyJobHandle};

use ai::{Considerations, DecisionWeight, Dse};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BuildDse {
    pub job: SocietyJobHandle,
    pub details: BuildDetails,
}

impl Dse<AiContext> for BuildDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        // TODO wants to work, can work
        // TODO has tool
        out.add(MyProximityToConsideration(AiTarget::Block(
            self.details.pos,
        )));
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn action(&self, _: &mut AiBlackboard, _: Option<AiTarget>) -> AiAction {
        AiAction::GoBuild {
            job: self.job,
            details: self.details.clone(),
        }
    }
}
