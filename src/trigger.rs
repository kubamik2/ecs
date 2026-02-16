use crate::{observer::TriggerInput, world::WorldPtr, Entity, Event};

pub struct Trigger<'a, E: Event> {
    event: &'a mut E,
    target: Option<Entity>,
}

impl<E: Event> Trigger<'_, E> {
    pub fn event(&self) -> &E {
        self.event
    }

    pub fn event_mut(&mut self) -> &mut E {
        self.event
    }

    pub fn target(&self) -> Option<Entity> {
        self.target
    }

    pub(crate) unsafe fn fetch(_: WorldPtr<'_>, trigger_input: TriggerInput) -> Trigger<'_, E> {
        Trigger {
            event: unsafe { trigger_input.event.cast::<E>().as_mut() },
            target: trigger_input.target,
        }
    }
}
