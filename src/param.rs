use crate::access::SignalAccess;

use super::{access::Access, query::QueryData, Query, ECS};

#[allow(unused_variables)]
pub trait SystemParam {
    type Item<'a>;
    fn create(ecs: &ECS) -> Option<Self::Item<'_>>;
    fn join_component_access(component_access: &mut Access) {}
    fn join_resource_access(resource_access: &mut Access) {}
}

impl<D: QueryData> SystemParam for Query<D> {
    type Item<'a> = Self;
    fn create(ecs: &ECS) -> Option<Self> {
        Query::new(ecs)
    }

    fn join_component_access(component_access: &mut Access) {
        D::join_component_access(component_access);
    }
}

#[allow(unused_variables)]
pub trait ObserverParam {
    type Item<'a>;
    fn create(ecs: &ECS) -> Option<Self::Item<'_>>;
    fn join_component_access(component_access: &mut Access) {}
    fn join_resource_access(resource_access: &mut Access) {}
    fn join_signal_access(signal_access: &mut SignalAccess) {}
}

impl<T: SystemParam> ObserverParam for T {
    type Item<'a> = T::Item<'a>;
    fn create(ecs: &ECS) -> Option<Self::Item<'_>> {
        T::create(ecs)
    }

    fn join_component_access(component_access: &mut Access) {
        T::join_component_access(component_access);
    }

    fn join_resource_access(resource_access: &mut Access) {
        T::join_resource_access(resource_access)
    }
}
