mod bitmap;
mod component_manager;
mod entity_manager;
mod system;
mod ecs;
mod query;

pub use ecs_derive::Component;
pub use ecs::ECS;
pub use query::Query;
pub use system::Schedule;

pub const MAX_ENTITIES: usize = 8192;
pub const MAX_COMPONENTS: usize = 32;

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
