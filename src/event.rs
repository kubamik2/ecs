use std::any::TypeId;
use crate::{param::SystemParam, world::WorldPtr, Res, ResMut, Resource};


#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EventId(u16);

pub trait Event: Send + Clone + Sync + 'static {}

pub struct EventQueue<E: Event> {
    old: Vec<E>,
    old_count: usize,
    new: Vec<E>,
    new_count: usize,
    count: usize,
}

impl<E: Event> Resource for EventQueue<E> {}

impl<E: Event> Default for EventQueue<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Event> EventQueue<E> {
    pub const fn new() -> Self {
        Self {
            old: Vec::new(),
            old_count: 0,
            new: Vec::new(),
            new_count: 0,
            count: 0,
        }
    }

    pub fn send(&mut self, event: E) {
        self.new.push(event);
        self.count += 1;
    }

    pub fn send_batch<I: IntoIterator<Item = E>>(&mut self, events: I) {
        let new_len = self.new.len();
        self.new.extend(events);
        let batch_size = self.new.len() - new_len;
        self.count += batch_size;
    }

    pub fn update(&mut self) {
        std::mem::swap(&mut self.new, &mut self.old);
        self.new.clear();
        self.old_count = self.new_count;
        self.new_count += self.old.len();
    }
}

pub struct EventReader<'a, E: Event> {
    last_count: &'a mut usize,
    event_queue: Res<'a, EventQueue<E>>,
}

impl<E: Event> EventReader<'_, E> {
    pub fn read(&mut self) -> EventIterator<E> {
        EventIterator::new(&self.event_queue, self.last_count)
    }
}

impl<E: Event> SystemParam for EventReader<'_, E> {
    type Item<'a> = EventReader<'a, E>;
    type State = usize;
    fn init_state(_: &mut crate::World) -> Self::State {
        0
    }

    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut usize) -> Self::Item<'a> {
        let event_queue = unsafe { world_ptr.as_world() }.resource::<EventQueue<E>>();
        EventReader {
            last_count: state,
            event_queue,
        }
    }

    fn join_resource_access(resource_access: &mut crate::access::Access) {
        resource_access.immutable.insert(TypeId::of::<EventQueue<E>>());
    }
}

pub struct EventReadWriter<'a, E: Event> {
    last_count: &'a mut usize,
    event_queue: ResMut<'a, EventQueue<E>>,
}

impl<E: Event> EventReadWriter<'_, E> {
    pub fn read(&mut self) -> EventIterator<E> {
        EventIterator::new(&self.event_queue, self.last_count)
    }

    pub fn send(&mut self, event: E) {
        self.event_queue.send(event);
    }

    pub fn send_batch<I: IntoIterator<Item = E>>(&mut self, events: I) {
        self.event_queue.send_batch(events);
    }
}

impl<E: Event> SystemParam for EventReadWriter<'_, E> {
    type Item<'a> = EventReadWriter<'a, E>;
    type State = usize;
    fn init_state(_: &mut crate::World) -> Self::State {
        0
    }

    unsafe fn fetch<'a>(mut world_ptr: WorldPtr<'a>, state: &'a mut usize) -> Self::Item<'a> {
        let event_queue = unsafe { world_ptr.as_world_mut() }.resource_mut::<EventQueue<E>>();
        EventReadWriter {
            last_count: state,
            event_queue,
        }
    }

    fn join_resource_access(resource_access: &mut crate::access::Access) {
        resource_access.mutable.insert(TypeId::of::<EventQueue<E>>());
        resource_access.mutable_count += 1;
    }
}

pub struct EventIterator<'a, E: Event> {
    iter: std::iter::Chain<std::slice::Iter<'a, E>, std::slice::Iter<'a, E>>,
    last_count: &'a mut usize,
}

impl <'a, E: Event> EventIterator<'a, E> {
    pub fn new(event_queue: &'a EventQueue<E>, last_count: &'a mut usize) -> Self {
        *last_count = (*last_count).max(event_queue.old_count);
        let queue_old_sliced_index = (*last_count - event_queue.old_count).min(event_queue.old.len());
        let queue_new_sliced_index = last_count.saturating_sub(event_queue.new_count).min(event_queue.new.len());
        
        let queue_old_sliced = &event_queue.old[queue_old_sliced_index..];
        let queue_new_sliced = &event_queue.new[queue_new_sliced_index..];

        Self {
            iter: queue_old_sliced.iter().chain(queue_new_sliced.iter()),
            last_count,
        }
    }
}

impl<'a, E: Event> Iterator for EventIterator<'a, E> {
    type Item = &'a E;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().inspect(|_| *self.last_count += 1)
    }
}
