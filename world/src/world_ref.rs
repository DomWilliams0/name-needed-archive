use crate::World;
use std::cell::{Ref, RefCell, RefMut};

use std::rc::Rc;

/// Reference counted reference to the world
#[derive(Clone)]
pub struct WorldRef(Rc<RefCell<World>>);

pub type InnerWorldRef<'a> = Ref<'a, World>;
pub type InnerWorldRefMut<'a> = RefMut<'a, World>;

impl WorldRef {
    pub fn new(world: World) -> Self {
        Self(Rc::new(RefCell::new(world)))
    }

    pub fn borrow(&self) -> InnerWorldRef<'_> {
        (*self.0).borrow()
    }

    pub fn borrow_mut(&self) -> InnerWorldRefMut<'_> {
        (*self.0).borrow_mut()
    }
}

// just to satisfy specs, never used because we don't use more than 1 thread
unsafe impl Send for WorldRef {}
unsafe impl Sync for WorldRef {}

// just to satisfy specs, never used
impl Default for WorldRef {
    fn default() -> Self {
        unimplemented!()
    }
}