use sortedvec::*;

use common::*;
use unit::space::length::Length3;
use unit::space::volume::Volume;

use crate::ecs::Entity;

use crate::item::inventory::container::contents::SortedContents;
use crate::item::inventory::HeldEntity;

mod contents {
    #![allow(clippy::toplevel_ref_arg, clippy::from_over_into)]

    use super::*;

    // TODO sort by some item type identifier so common items are grouped together
    // Cow<str> of the item name?
    sortedvec! {
        pub struct SortedContents {
            fn derive_key(entity: &HeldEntity) -> Entity { entity.entity }
        }
    }

    impl SortedContents {
        pub(in crate::item::inventory) fn inner_mut(&mut self) -> &mut Vec<HeldEntity> {
            &mut self.inner
        }

        pub(in crate::item::inventory) fn inner(&self) -> &[HeldEntity] {
            &self.inner
        }
    }
}

pub struct Container {
    /// Maximum total volume of all contents
    volume_limit: Volume,

    /// Size limit on adding individual items, does not accumulate
    size_limit: Length3,

    contents: SortedContents,
    current_volume: Volume,
}

#[derive(Debug, Error, Clone)]
pub enum ContainerError {
    #[error("Item is too big")]
    TooBig,

    #[error("Container is full")]
    Full,

    #[error("Container does not contain {}", *.0)]
    NotFound(Entity),
}

impl Container {
    pub fn new(volume_limit: Volume, size_limit: Length3) -> Self {
        Container {
            volume_limit,
            size_limit,
            contents: SortedContents::default(),
            current_volume: Volume::new(0),
        }
    }
    pub fn add_with(
        &mut self,
        entity: Entity,
        entity_volume: Volume,
        entity_size: Length3,
    ) -> Result<(), ContainerError> {
        let held = HeldEntity {
            entity,
            volume: entity_volume,
            size: entity_size,
        };

        self.add(&held)
    }

    /// Returns new volume
    pub fn fits(&self, size: Length3, volume: Volume) -> Result<Volume, ContainerError> {
        if !self.size_limit.fits(size) {
            return Err(ContainerError::TooBig);
        }

        let new_volume = self.current_volume + volume;
        if new_volume > self.volume_limit {
            return Err(ContainerError::Full);
        }

        // fits
        Ok(new_volume)
    }

    /// Clones on successful add and returns Ok
    pub fn add(&mut self, entity: &HeldEntity) -> Result<(), ContainerError> {
        let new_volume = self.fits(entity.size, entity.volume)?;

        self.current_volume = new_volume;
        self.contents.insert(entity.to_owned());

        Ok(())
    }

    pub fn remove(&mut self, entity: Entity) -> Result<(), ContainerError> {
        match self.contents.remove(&entity) {
            None => Err(ContainerError::NotFound(entity)),
            Some(entity) => {
                self.current_volume -= entity.volume;
                Ok(())
            }
        }
    }

    pub fn remove_at_index(&mut self, idx: usize) -> HeldEntity {
        let entity = self.contents.inner_mut().remove(idx);
        self.current_volume -= entity.volume;
        entity
    }

    pub fn remove_all(&mut self) -> impl Iterator<Item = HeldEntity> + '_ {
        self.current_volume = Volume::new(0);
        self.contents.inner_mut().drain(..)
    }

    pub(in crate::item::inventory) fn contents_as_slice(&self) -> &[HeldEntity] {
        self.contents.inner()
    }

    /// Sorted by entity
    pub fn contents(&self) -> impl Iterator<Item = &HeldEntity> + '_ {
        self.contents.iter()
    }

    pub fn limits(&self) -> (Volume, Length3) {
        (self.volume_limit, self.size_limit)
    }

    pub fn current_capacity(&self) -> Volume {
        self.current_volume
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.contents.iter().any(|e| e.entity == entity)
    }
}
