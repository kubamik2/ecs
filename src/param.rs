use crate::access::SignalAccess;

use super::{access::Access, ECS};

#[allow(unused_variables)]
pub trait SystemParam {
    type Item<'a>;
    type State: Send + Sync;
    fn join_component_access(component_access: &mut Access) {}
    fn join_resource_access(resource_access: &mut Access) {}
    fn init_state(ecs: &mut ECS) -> Self::State;
    fn fetch<'a>(ecs: &'a ECS, state: &'a mut Self::State) -> Self::Item<'a>;
}


#[allow(unused_variables)]
pub trait ObserverParam {
    type Item<'a>;
    type State: Send + Sync;
    fn join_component_access(component_access: &mut Access) {}
    fn join_resource_access(resource_access: &mut Access) {}
    fn join_signal_access(signal_access: &mut SignalAccess) {}
    fn init_state(ecs: &mut ECS) -> Self::State;
    fn fetch<'a>(ecs: &'a ECS, state: &'a mut Self::State) -> Self::Item<'a>;
}

impl<T: SystemParam> ObserverParam for T {
    type Item<'a> = T::Item<'a>;
    type State = T::State;

    fn fetch<'a>(ecs: &'a ECS, state: &'a mut Self::State) -> Self::Item<'a> {
        T::fetch(ecs, state)
    }

    fn init_state(ecs: &mut ECS) -> Self::State {
        T::init_state(ecs)
    }

    fn join_component_access(component_access: &mut Access) {
        T::join_component_access(component_access);
    }

    fn join_resource_access(resource_access: &mut Access) {
        T::join_resource_access(resource_access)
    }
}
