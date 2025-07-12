#![feature(sync_unsafe_cell)]
#![allow(clippy::too_many_arguments)]
mod bitmap;
mod component_manager;
mod entity_manager;
pub mod system;
mod ecs;
mod query;
mod param;
mod access;
mod resource;

pub use ecs_derive::{Component, Resource};
pub use ecs::ECS;
pub use query::Query;
pub use system::Schedule;
pub use resource::{Res, ResMut};

pub const MAX_ENTITIES: usize = 8192;
pub const MAX_COMPONENTS: usize = 32;
pub const MAX_RESOURCES: usize = 32;

#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entity(u16);

impl Entity {
    pub(crate) const fn new(id: u16) -> Self {
        Self(id)
    }

    pub const fn id(&self) -> u16 {
        self.0
    }
}

pub trait Component {
    fn signature_index() -> usize;
}

pub trait Resource {
    fn signature_index() -> usize;
}
