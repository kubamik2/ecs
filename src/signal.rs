use crate::{observer::SignalInput, world::WorldPtr, Entity, Event};

pub struct Signal<'a, E: Event> {
    event: &'a mut E,
    target: Option<Entity>,
}

impl<E: Event> Signal<'_, E> {
    pub fn event(&self) -> &E {
        self.event
    }

    pub fn event_mut(&mut self) -> &mut E {
        self.event
    }

    pub fn target(&self) -> Option<Entity> {
        self.target
    }

    pub(crate) unsafe fn fetch(_: WorldPtr<'_>, signal_input: SignalInput) -> Signal<'_, E> {
        Signal {
            event: unsafe { signal_input.event.cast::<E>().as_mut() },
            target: signal_input.target,
        }
    }
}
