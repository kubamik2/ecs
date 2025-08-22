use std::{any::TypeId, borrow::Borrow};

use crate::{param::ObserverParam, Entity, Event, Res, Resource};

pub struct Signal<'a, E: Event> {
    event_index: usize,
    signal_queue: Res<'a, SignalQueue<E>>,
}

impl<E: Event> Signal<'_, E> {
    pub fn event(&self) -> &E {
        &self.signal_queue.queue[self.event_index].event
    }

    pub fn target(&self) -> Option<Entity> {
        self.signal_queue.queue[self.event_index].target
    }
}

impl<E: Event> ObserverParam for Signal<'_, E> {
    type Item<'a> = Signal<'a, E>;
    type State = usize;

    fn join_signal_access(signal_access: &mut crate::access::SignalAccess) {
        signal_access.required.insert(TypeId::of::<E>());
    }

    fn init_state(world: &mut crate::World) -> Self::State {
        let signal_queue = world.resource::<SignalQueue<E>>();
        signal_queue.current_event
    }

    fn fetch<'a>(world: &'a crate::World, state: &'a mut Self::State) -> Self::Item<'a> {
        let signal_queue = world.resource::<SignalQueue<E>>();
        Signal {
            event_index: *state,
            signal_queue,
        }
    }
}

struct OwnedSignal<E: Event> {
    event: E,
    target: Option<Entity>,
}


pub(crate) struct SignalQueue<E: Event> {
    queue: Vec<OwnedSignal<E>>,
    current_event: usize,
}

impl<E: Event> Default for SignalQueue<E> {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            current_event: 0,
        }
    }
}

impl<E: Event> SignalQueue<E> {
    pub(crate) fn send(&mut self, event: E) {
        self.queue.push(OwnedSignal { event, target: None });
    }

    pub(crate) fn send_targeted(&mut self, event: E, target: Entity) {
        self.queue.push(OwnedSignal { event, target: Some(target) });
    }
}

impl<E: Event> Resource for SignalQueue<E> {}
