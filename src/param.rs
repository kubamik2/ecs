use std::any::TypeId;

use crate::{Commands, system::SystemHandle, world::WorldPtr};

use super::{access::Access, World};

#[allow(unused_variables)]
pub trait SystemParam {
    type Item<'a>;
    type State: Send + Sync;
    fn join_component_access(world: &mut World, component_access: &mut Access) {}
    fn join_resource_access(world: &mut World, resource_access: &mut Access) {}
    fn join_signal_access(signal_access: &mut Option<TypeId>) {}
    fn init_state(world: &mut World) -> Self::State;
    /// This function will run in parallel
    /// It is only meant to fetch the reference to the data from the world
    /// # Safety
    /// The caller must not modify the world such that it would cause a data race
    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut Self::State, system_meta: &'a SystemHandle<'a>) -> Self::Item<'a>;
    fn after<'state>(commands: &mut Commands<'state>, state: &'state mut Self::State) {}
}
