use crate::access::SignalAccess;

use super::{access::Access, World};

#[allow(unused_variables)]
pub trait SystemParam {
    type Item<'a>;
    type State: Send + Sync;
    fn join_component_access(component_access: &mut Access) {}
    fn join_resource_access(resource_access: &mut Access) {}
    fn init_state(world: &mut World) -> Self::State;
    fn fetch<'a>(world: &'a World, state: &'a mut Self::State) -> Self::Item<'a>;
}


#[allow(unused_variables)]
pub trait ObserverParam {
    type Item<'a>;
    type State: Send + Sync;
    fn join_component_access(component_access: &mut Access) {}
    fn join_resource_access(resource_access: &mut Access) {}
    fn join_signal_access(signal_access: &mut SignalAccess) {}
    fn init_state(world: &mut World) -> Self::State;
    fn fetch<'a>(world: &'a World, state: &'a mut Self::State) -> Self::Item<'a>;
}

impl<T: SystemParam> ObserverParam for T {
    type Item<'a> = T::Item<'a>;
    type State = T::State;

    fn fetch<'a>(world: &'a World, state: &'a mut Self::State) -> Self::Item<'a> {
        T::fetch(world, state)
    }

    fn init_state(world: &mut World) -> Self::State {
        T::init_state(world)
    }

    fn join_component_access(component_access: &mut Access) {
        T::join_component_access(component_access);
    }

    fn join_resource_access(resource_access: &mut Access) {
        T::join_resource_access(resource_access)
    }
}
