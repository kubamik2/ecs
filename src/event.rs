use std::marker::PhantomData;

use crate::{Resource, World, param::{SystemParam, SystemParamError, get_resource_id}, system::SystemHandle, world::WorldPtr};

pub struct EventQueue<E: Send + Sync + 'static> {
    old: Vec<E>,
    old_count: usize,
    new: Vec<E>,
    new_count: usize,
    count: usize,
}

impl<E: Send + Sync + 'static> Resource for EventQueue<E> {}

impl<E: Send + Sync + 'static> Default for EventQueue<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Send + Sync + 'static> EventQueue<E> {
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

pub struct EventReader<'a, E: Send + Sync + 'static> {
    last_count: &'a mut usize,
    event_queue: &'a EventQueue<E>,
}

impl<E: Send + Sync + 'static> EventReader<'_, E> {
    pub fn read(&mut self) -> EventIterator<'_, E> {
        EventIterator::new(self.event_queue, self.last_count)
    }
}

unsafe impl<E: Send + Sync + 'static> SystemParam for EventReader<'_, E> {
    type Item<'a> = EventReader<'a, E>;
    type State = usize;
    fn init_state(_: &mut crate::World, _: &SystemHandle) -> Result<Self::State, SystemParamError> {
        Ok(0)
    }

    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut usize, _: &SystemHandle) -> Self::Item<'a> {
        let event_queue = unsafe { world_ptr.as_world() }.resource::<EventQueue<E>>();
        EventReader {
            last_count: state,
            event_queue,
        }
    }

    fn join_resource_access(world: &mut World, resource_access: &mut crate::access::Access) -> Result<(), SystemParamError> {
        resource_access.add_immutable(get_resource_id::<EventQueue<E>>(world)?.get());
        Ok(())
    }
}

pub struct EventReadWriter<'a, E: Send + Sync + 'static> {
    last_count: &'a mut usize,
    event_queue: &'a mut EventQueue<E>,
}

impl<E: Send + Sync + 'static> EventReadWriter<'_, E> {
    pub fn read(&mut self) -> EventIterator<'_, E> {
        EventIterator::new(self.event_queue, self.last_count)
    }

    pub fn send(&mut self, event: E) {
        self.event_queue.send(event);
    }

    pub fn send_batch<I: IntoIterator<Item = E>>(&mut self, events: I) {
        self.event_queue.send_batch(events);
    }
}

unsafe impl<E: Send + Sync + 'static> SystemParam for EventReadWriter<'_, E> {
    type Item<'a> = EventReadWriter<'a, E>;
    type State = usize;
    fn init_state(_: &mut crate::World, _: &SystemHandle) -> Result<Self::State, SystemParamError> {
        Ok(0)
    }

    unsafe fn fetch<'a>(mut world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        let event_queue = unsafe { world_ptr.as_world_mut() }.resource_ref_mut::<EventQueue<E>>();
        EventReadWriter {
            last_count: state,
            event_queue,
        }
    }

    fn join_resource_access(world: &mut World, resource_access: &mut crate::access::Access) -> Result<(), SystemParamError> {
        resource_access.add_mutable(get_resource_id::<EventQueue<E>>(world)?.get());
        Ok(())
    }
}

pub struct EventIterator<'a, E: Send + Sync + 'static> {
    iter: std::iter::Chain<std::slice::Iter<'a, E>, std::slice::Iter<'a, E>>,
    last_count: &'a mut usize,
}

impl <'a, E: Send + Sync + 'static> EventIterator<'a, E> {
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

impl<'a, E: Send + Sync + 'static> Iterator for EventIterator<'a, E> {
    type Item = &'a E;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().inspect(|_| *self.last_count += 1)
    }
}

#[derive(Default)]
pub struct EventReaderState<E: Send + Sync + 'static> {
    last_count: usize,
    _m: PhantomData<E>,
}

impl<E: Send + Sync + 'static> EventReaderState<E> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            last_count: 0,
            _m: PhantomData,
        }
    }

    pub fn reader<'a>(&'a mut self, world: &'a World) -> Result<EventReader<'a, E>, SystemParamError> {
        Ok(EventReader {
            last_count: &mut self.last_count,
            event_queue: world.get_resource().ok_or(SystemParamError::MissingResource(std::any::type_name::<EventQueue<E>>()))?,
        })
    }
}
