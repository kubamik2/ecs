use std::any::TypeId;

use crate::world::WorldPtr;

use super::{access::Access, World};

#[allow(unused_variables)]
pub trait SystemParam {
    type Item<'a>;
    type State: Send + Sync;
    fn join_component_access(component_access: &mut Access) {}
    fn join_resource_access(resource_access: &mut Access) {}
    fn join_signal_access(signal_access: &mut Option<TypeId>) {}
    fn init_state(world: &mut World) -> Self::State;
    /// This function will run in parallel
    /// # Safety
    /// The caller must not modify the world such that it would cause a data race
    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut Self::State) -> Self::Item<'a>;
    fn after(world: &mut World, state: &mut Self::State) {}
}
