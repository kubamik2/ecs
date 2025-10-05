use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};

use crate::{Commands, storage::sparse_set::SparseSet};

pub const MAX_ENTITIES: usize = u16::MAX as usize;

#[derive(Hash, Clone, Copy, PartialEq, Eq)]
pub struct Entity {
    id: u16,
    version: u16,
}

impl Entity {
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
}

impl std::fmt::Debug for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}v{}", self.id(), self.version()))
    }
}

pub struct Entities {
    free_entity_ids: Vec<u16>,
    entity_versions: Vec<u16>,
    claimed_free_entities: AtomicUsize,
    highest_free_entity_id: AtomicU16,
    // TODO might want to consider a different structure than Vec for fast removal and unique insertion
    children: SparseSet<Vec<Entity>>,
}

impl Default for Entities {
    fn default() -> Self {
        Self {
            free_entity_ids: Vec::new(),
            entity_versions: Vec::new(),
            claimed_free_entities: AtomicUsize::new(0),
            highest_free_entity_id: AtomicU16::new(0),
            children: SparseSet::default(),
        }
    }
}

impl Entities {
    pub fn despawn(&mut self, entity: Entity, command_buffer: &mut Vec<u8>) {
        for _ in 0..self.claimed_free_entities.load(Ordering::Relaxed).min(self.free_entity_ids.len()) {
            self.free_entity_ids.pop();
        }
        self.claimed_free_entities.store(0, Ordering::Relaxed);
        self.free_entity_ids.push(entity.id());
        if entity.id() as usize >= self.entity_versions.len() {
            self.entity_versions.resize(entity.id() as usize + 1, 0);
        }
        self.entity_versions[entity.id() as usize] += 1;

        if let Some(children) = self.children.remove(entity.id() as usize) {
            let mut commands = Commands::new(command_buffer, self);
            for entity in children {
                commands.despawn(entity);
            }
        }
    }

    pub fn spawn(&self) -> Entity {
        if self.free_entity_ids.is_empty() {
            let highest_free_entity_id = self.highest_free_entity_id.fetch_add(1, Ordering::Relaxed);
            Entity::new(highest_free_entity_id, 0)
        } else {
            let free_entity_index = (self.free_entity_ids.len() - 1).checked_sub(self.claimed_free_entities.fetch_add(1, Ordering::Relaxed));
            if let Some(free_entity_index) = free_entity_index {
                let entity_id = self.free_entity_ids[free_entity_index];
                let version = self.entity_versions[entity_id as usize];
                Entity::new(entity_id, version)
            } else {
                let highest_free_entity_id = self.highest_free_entity_id.fetch_add(1, Ordering::Relaxed);
                Entity::new(highest_free_entity_id, 0)
            }
        }
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        let Some(version) = self.entity_versions.get(entity.id() as usize) else { return true; };
        entity.version() == *version
    }


    pub fn add_child(&mut self, parent: Entity, child: Entity) {
        assert_ne!(parent, child, "entity '{:?}' tried to be it's own child", parent);
        let children = self.children.entry(parent.id() as usize).or_default();
        if !children.contains(&child) {
            children.push(child);
        }
    }

    pub fn remove_child(&mut self, parent: Entity, child: Entity) {
        let children = self.children.entry(parent.id() as usize).or_default();
        if let Some(index) = children.iter().position(|p| child == *p) {
            children.swap_remove(index);
        }
    }

    pub fn remove_children(&mut self, parent: Entity) {
        self.children.remove(parent.id() as usize);
    }

    pub fn children(&self, entity: Entity) -> &[Entity] {
        if let Some(children) = self.children.get(entity.id() as usize) {
            children
        } else {
            &[]
        }
    }
}
