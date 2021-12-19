use crate::ecs::*;
use crate::job::SocietyJobHandle;
use crate::BlockType;
use common::*;

mod world_helper;

// TODO organise build module

#[derive(Hash, Clone, Eq, PartialEq)]
pub struct BuildMaterial {
    // TODO flexible list of reqs based on components
    definition_name: &'static str,
    quantity: u16,
}

pub trait Build: Debug {
    /// Target block
    fn output(&self) -> BlockType;

    /// (number of steps required, ticks to sleep between each step)
    fn progression(&self) -> (u32, u32);

    // TODO can this somehow return an iterator of build materials?
    fn materials(&self, materials_out: &mut Vec<BuildMaterial>);
}

/// Reserved for a build job
#[derive(Component, EcsComponent, Debug)]
#[storage(HashMapStorage)]
#[name("reserved-material")]
#[clone(disallow)]
pub struct ReservedMaterialComponent {
    pub build_job: SocietyJobHandle,
}

/// In the process of being consumed for a build job
#[derive(Component, EcsComponent, Debug, Default)]
#[storage(NullStorage)]
#[name("consumed-material")]
#[clone(disallow)]
pub struct ConsumedMaterialForJobComponent;

impl BuildMaterial {
    /// Quantity must be >0
    pub fn new(definition_name: &'static str, quantity: u16) -> Self {
        // TODO use NonZeroU16
        assert!(quantity > 0);
        Self {
            definition_name,
            quantity,
        }
    }

    pub fn definition(&self) -> &'static str {
        self.definition_name
    }

    pub fn quantity(&self) -> u16 {
        self.quantity
    }
}

impl Debug for BuildMaterial {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.quantity, self.definition_name)
    }
}

#[derive(Debug)]
pub struct StoneBrickWall;

impl Build for StoneBrickWall {
    fn output(&self) -> BlockType {
        BlockType::StoneBrickWall
    }

    fn progression(&self) -> (u32, u32) {
        (20, 4)
    }

    fn materials(&self, materials_out: &mut Vec<BuildMaterial>) {
        materials_out.push(BuildMaterial::new("core_brick_stone", 6))
    }
}
