use crate::bitmap::Bitmap;
use super::{Component, World};

// -1 accounts for the invalid entity
pub const MAX_ENTITIES: usize = u16::MAX as usize - 1;

#[derive(Hash, Clone, Copy, PartialEq, Eq)]
pub struct Entity {
    id: u16,
    version: u16,
}

impl Entity {
    const INVALID: Entity = Entity { id: u16::MAX, version: u16::MAX };

    #[inline(always)]
    const fn new(id: u16, version: u16) -> Self {
        assert!(id as usize <= MAX_ENTITIES);
        Self { id, version }
    }

    #[inline(always)]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[inline(always)]
    pub const fn id(&self) -> u16 {
        self.id
    }

    #[inline(always)]
    pub(crate) const fn increment_version(&mut self) {
        self.version += 1;
    }
}

impl std::fmt::Debug for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}v{}", self.id(), self.version()))
    }
}

pub struct Entities {
    list: Vec<Entity>,
    next: Entity,
    available: usize,
}

impl Default for Entities {
    fn default() -> Self {
        Self {
            list: vec![Entity::new(0,0)],
            next: Entity::INVALID,
            available: 0,
        }
    }
}

impl Entities {
    #[inline]
    pub const fn new() -> Self {
        Self {
            list: Vec::new(),
            next: Entity::INVALID,
            available: 0,
        }
    }

    pub fn despawn(&mut self, mut entity: Entity) {
        self.available += 1;
        self.list[entity.id() as usize] = self.next;
        entity.increment_version();
        self.next = entity;
    }

    pub fn spawn(&mut self) -> Entity {
        assert!(self.list.len() != MAX_ENTITIES, "max entities reached");
        if self.available > 0 {
            let entity = self.next;
            let next = entity.id();
            self.next = self.list[next as usize];
            self.list[entity.id() as usize] = entity;
            self.available -= 1;
            debug_assert!(entity != Entity::INVALID);
            entity
        } else {
            let entity = Entity::new(self.list.len() as u16, 0);
            self.list.push(entity);
            entity
        }
    }

    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.list[entity.id() as usize] == entity
    }
}

pub trait EntityBundle {
    fn spawn(self, world: &mut World) -> Entity;
}

impl<C: Component + 'static> EntityBundle for C {
    fn spawn(self, world: &mut World) -> Entity {
        let signature = world.register_component::<C>().as_signature();
        unsafe {
            let entity = world.spawn_with_signature(signature);
            world.set_component_unchecked(entity, self);
            entity
        }
    }
}

macro_rules! bundle_typle_impl {
    ($(($idx:tt, $name:ident)),+) => {
        impl<$($name: Component + 'static),+> EntityBundle for ($($name),+) {
            fn spawn(self, world: &mut World) -> Entity {
                let data = self;
                let mut signature = Bitmap::new();
                $(signature |= world.register_component::<$name>().as_signature();)+
                unsafe {
                    let entity = world.spawn_with_signature(signature);
                    $(world.set_component_unchecked(entity, data.$idx));+;
                    entity
                }
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{bundle_typle_impl, 2, 32, C}
