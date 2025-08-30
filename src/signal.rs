use crate::{world::WorldPtr, Entity, Event, Res, Resource};

pub struct Signal<'a, E: Event> {
    signal_index: usize,
    signal_queue: Res<'a, SignalQueue<E>>,
}

impl<E: Event> Signal<'_, E> {
    pub fn event(&self) -> &E {
        &self.signal_queue.queue[self.signal_index].event
    }

    pub fn target(&self) -> Option<Entity> {
        self.signal_queue.queue[self.signal_index].target
    }

    /// This function will run in parallel
    /// # Safety
    /// The caller must not modify the world such that it would cause a data race
    pub(crate) unsafe fn fetch(world_ptr: WorldPtr<'_>, signal_index: usize) -> Signal<'_, E> {
        let signal_queue = unsafe { world_ptr.as_world() }.resource::<SignalQueue<E>>();
        Signal {
            signal_queue,
            signal_index,
        }
    }
}

struct OwnedSignal<E: Event> {
    event: E,
    target: Option<Entity>,
}


pub(crate) struct SignalQueue<E: Event> {
    queue: Vec<OwnedSignal<E>>,
}

impl<E: Event> Default for SignalQueue<E> {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
        }
    }
}

impl<E: Event> SignalQueue<E> {
    #[inline]
    pub(crate) fn send(&mut self, event: E, target: Option<Entity>) {
        self.queue.push(OwnedSignal { event, target });
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.queue.len()
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.queue.clear();
    }
}

impl<E: Event> Resource for SignalQueue<E> {}
