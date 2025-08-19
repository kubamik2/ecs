use std::any::TypeId;

use crate::param::ObserverParam;

pub struct Signal<'a, E: 'static> {
    event: &'a E,
}

impl<E: 'static> ObserverParam for Signal<'_, E> {
    type Item<'a> = Signal<'a, E>;
    fn create(ecs: &crate::ECS) -> Option<Self::Item<'_>> {
        todo!()
    }

    fn join_signal_access(signal_access: &mut crate::access::SignalAccess) {
        signal_access.required.insert(TypeId::of::<E>());
    }
}
