use crate::bitmap::Bitmap;

use super::{Component, Entity, ECS, MAX_ENTITIES};

pub struct EntityManager {
    entities: Vec<Entity>,
    next: Entity,
    available: usize,
}

impl Default for EntityManager {
    fn default() -> Self {
        Self {
            entities: vec![Entity::new(0,0)],
            next: Entity::new(0,0),
            available: 0,
        }
    }
}

impl EntityManager {
    pub fn remove(&mut self, mut entity: Entity) {
        self.available += 1;
        self.entities[entity.id() as usize] = self.next;
        entity.increment_version();
        self.next = entity;
    }

    pub fn spawn(&mut self) -> Entity {
        assert!(self.entities.len() != MAX_ENTITIES);
        if self.available > 0 {
            let entity = self.next;
            let next = entity.id();
            self.next = self.entities[next as usize];
            self.entities[entity.id() as usize] = entity;
            self.available -= 1;
            entity
        } else {
            let entity = Entity::new(self.entities.len() as u16, 0);
            self.entities.push(entity);
            entity
        }
    }

    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities[entity.id() as usize] == entity
    }
}

pub trait EntityBundle {
    type Data;
    fn spawn(self, ecs: &mut ECS) -> Entity;
}

impl<C: Component + 'static> EntityBundle for C {
    type Data = C;
    fn spawn(self, ecs: &mut ECS) -> Entity {
        let entity = ecs.entity_manager.spawn();
        let signature = ecs.component_manager.register_component::<C>();
        ecs.component_manager.spawn_entity(entity, signature);
        unsafe { ecs.component_manager.set_component_limited_checks(entity, self) };
        entity
    }
}

macro_rules! bundle_typle_impl {
    ($(($idx:tt, $name:ident)),+) => {
        impl<$($name: Component + 'static),+> EntityBundle for ($($name),+) {
            type Data = ($($name),+);
            fn spawn(self, ecs: &mut ECS) -> Entity {
                let data = self;
                let mut signature = Bitmap::new();
                $(signature |= ecs.component_manager.register_component::<$name>();)+
                let entity = ecs.entity_manager.spawn();
                ecs.component_manager.spawn_entity(entity, signature);
                unsafe {
                    $(ecs.component_manager.set_component_limited_checks(entity, data.$idx));+;
                }
                entity
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{bundle_typle_impl, 2, 32, C}
