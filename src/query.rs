use crate::{param::SystemParam, system::SystemHandle, world::WorldPtr, ComponentId, Signature};
use super::{access::Access, bitmap::Bitmap, Component, Entity, World};
use std::{any::TypeId, collections::HashSet, mem::MaybeUninit};

const QUERY_MAX_VARIADIC_COUNT: usize = 32;

pub struct Query<'a, D: QueryData> {
    _a: std::marker::PhantomData<D>,
    world_ptr: WorldPtr<'a>,
    component_signature: Signature,
    cached_component_ids: [ComponentId; QUERY_MAX_VARIADIC_COUNT],
}

impl<D: QueryData> Clone for Query<'_, D> {
    fn clone(&self) -> Self {
        Self {
            _a: Default::default(),
            world_ptr: self.world_ptr,
            component_signature: self.component_signature,
            cached_component_ids: self.cached_component_ids,
        }
    }
}

impl<'a, D: QueryData> Query<'a, D> {
    pub fn new(world: &'a mut World) -> Self {
        D::register_components(world);
        let mut component_signature = Bitmap::new();
        let mut component_signature_map = HashSet::new();
        D::join_required_component_signature(&mut component_signature_map);
        for type_id in component_signature_map {
            let signature = world.get_component_signature_by_type_id(&type_id).expect("Query::new component not registered");
            component_signature |= signature;
        }
        let cached_component_ids = D::cache_component_ids(world);
        Self {
            _a: std::marker::PhantomData,
            world_ptr: world.world_ptr_mut(),
            component_signature,
            cached_component_ids,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = D::ItemRef<'a>> {
        let queue_signature = self.component_signature;
        let world_ptr = self.world_ptr;
        unsafe { world_ptr.as_world() }
            .groups()
            .iter()
            .filter(move |(signature, _)| {
                let signature = **signature;
                signature & queue_signature == queue_signature
            })
            .flat_map(|(_, entities)| entities.iter().copied())
            .map(|entity| unsafe { D::fetch_ref(self.world_ptr, entity, &self.cached_component_ids) })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = D::ItemMut<'a>> {
        let queue_signature = self.component_signature;
        unsafe { self.world_ptr.as_world() }
            .groups()
            .iter()
            .filter(move |(signature, _)| {
                let signature = **signature;
                signature & queue_signature == queue_signature
            })
            .flat_map(|(_, entities)| entities.iter().copied())
            .map(|entity| unsafe { D::fetch_mut(self.world_ptr, entity, &self.cached_component_ids) })
    }

    /// # Safety
    /// can violate rust's reference rules
    pub unsafe fn iter_unsafe(&self) -> impl Iterator<Item = D::ItemMut<'a>> {
        let queue_signature = self.component_signature;
        unsafe { self.world_ptr.as_world() }
            .groups()
            .iter()
            .filter(move |(signature, _)| {
                let signature = **signature;
                signature & queue_signature == queue_signature
            })
            .flat_map(|(_, entities)| entities.iter().copied())
            .map(|entity| unsafe { D::fetch_mut(self.world_ptr, entity, &self.cached_component_ids) })
    }

    pub fn get(&self, entity: Entity) -> Option<D::ItemRef<'_>> {
        let entity_signature = unsafe { self.world_ptr.as_world() }.get_entity_signature(entity)?;
        if (entity_signature & self.component_signature) != self.component_signature {
            return None;
        }
        Some(unsafe { D::fetch_ref(self.world_ptr, entity, &self.cached_component_ids) })
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<D::ItemMut<'_>> {
        let entity_signature = unsafe { self.world_ptr.as_world() }.get_entity_signature(entity)?;
        if (entity_signature & self.component_signature) != self.component_signature {
            return None;
        }
        Some(unsafe { D::fetch_mut(self.world_ptr, entity, &self.cached_component_ids) })
    }

    /// # Safety
    ///
    /// Might violate rust's reference rules
    pub unsafe fn get_unsafe(&self, entity: Entity) -> Option<D::ItemMut<'_>> {
        let entity_signature = unsafe { self.world_ptr.as_world() }.get_entity_signature(entity)?;
        if (entity_signature & self.component_signature) != self.component_signature {
            return None;
        }
        Some(unsafe { D::fetch_mut(self.world_ptr, entity, &self.cached_component_ids) })
    }
}

impl<D: QueryData> SystemParam for Query<'_, D> {
    type Item<'a> = Query<'a, D>;
    type State = (Signature, [ComponentId; QUERY_MAX_VARIADIC_COUNT]);

    fn join_component_access(world: &mut World, component_access: &mut Access) {
        D::join_component_access(world, component_access);
    }

    fn init_state(world: &mut World) -> Self::State {
        let query = Query::<D>::new(world);
        (query.component_signature, query.cached_component_ids)
    }

    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        Query {
            _a: Default::default(),
            cached_component_ids: state.1,
            component_signature: state.0,
            world_ptr,
        }
    }
}

pub trait QueryItem: Send + Sync {
    type ItemRef<'a>;
    type ItemMut<'a>;
    unsafe fn fetch_ref(world_ptr: WorldPtr<'_>, entity: Entity, component_index: ComponentId) -> Self::ItemRef<'_>;
    unsafe fn fetch_mut(world_ptr: WorldPtr<'_>, entity: Entity, component_index: ComponentId) -> Self::ItemMut<'_>;
    fn component_id_or_init(world: &mut World) -> ComponentId;
    fn component_id(_: &World) -> ComponentId;
    fn join_component_signature(_: &mut HashSet<TypeId>) {}
    fn join_component_access(_: &mut World, _: &mut Access) {}
    fn join_required_component_signature(_: &mut HashSet<TypeId>) {}
}

impl<C: Component> QueryItem for &C {
    type ItemRef<'a> = &'a C;
    type ItemMut<'a> = &'a C;
    #[inline]
    unsafe fn fetch_ref(world_ptr: WorldPtr<'_>, entity: Entity, component_id: ComponentId) -> Self::ItemRef<'_> {
        unsafe { world_ptr.as_world().get_component_by_id_unchecked::<C>(entity, component_id) }
    }

    #[inline]
    unsafe fn fetch_mut(world_ptr: WorldPtr<'_>, entity: Entity, component_id: ComponentId) -> Self::ItemMut<'_> {
        unsafe { world_ptr.as_world().get_component_by_id_unchecked::<C>(entity, component_id) }
    }

    fn component_id_or_init(world: &mut World) -> ComponentId {
        world.register_component::<C>()
    }
    
    #[inline]
    fn component_id(world: &World) -> ComponentId {
        world.get_component_id::<C>().expect("QueryItem component index not found")
    }

    #[inline]
    fn join_component_signature(component_signature: &mut HashSet<TypeId>) {
        component_signature.insert(TypeId::of::<C>());
    }

    #[inline]
    fn join_required_component_signature(component_signature: &mut HashSet<TypeId>) {
        component_signature.insert(TypeId::of::<C>());
    }

    #[inline]
    fn join_component_access(world: &mut World, component_access: &mut Access) {
        component_access.immutable.set(world.register_component::<C>().id());
    }
}

impl<C: Component> QueryItem for &mut C {
    type ItemRef<'a> = &'a C;
    type ItemMut<'a> = &'a mut C;
    #[inline]
    unsafe fn fetch_ref(world_ptr: WorldPtr<'_>, entity: Entity, component_index: ComponentId) -> Self::ItemRef<'_> {
        unsafe { world_ptr.as_world().get_component_by_id_unchecked::<C>(entity, component_index) }
    }

    #[inline]
    unsafe fn fetch_mut(mut world_ptr: WorldPtr<'_>, entity: Entity, component_index: ComponentId) -> Self::ItemMut<'_> {
        unsafe { world_ptr.as_world_mut().get_mut_component_by_id_unchecked::<C>(entity, component_index) }
    }

    fn component_id_or_init(world: &mut World) -> ComponentId {
        world.register_component::<C>()
    }
    
    #[inline]
    fn component_id(world: &World) -> ComponentId {
        world.get_component_id::<C>().expect("QueryItem component index not found")
    }

    #[inline]
    fn join_component_signature(component_signature: &mut HashSet<TypeId>) {
        component_signature.insert(TypeId::of::<C>());
    }

    #[inline]
    fn join_required_component_signature(component_signature: &mut HashSet<TypeId>) {
        component_signature.insert(TypeId::of::<C>());
    }

    #[inline]
    fn join_component_access(world: &mut World, component_access: &mut Access) {
        component_access.mutable.set(world.register_component::<C>().id());
        component_access.mutable_count += 1;
    }
}

impl QueryItem for Entity {
    type ItemRef<'a> = Entity;
    type ItemMut<'a> = Entity;

    #[inline]
    unsafe fn fetch_ref(_: WorldPtr<'_>, entity: Entity, _: ComponentId) -> Self::ItemRef<'_> {
        entity
    }

    #[inline]
    unsafe fn fetch_mut(_: WorldPtr<'_>, entity: Entity, _: ComponentId) -> Self::ItemMut<'_> {
        entity
    }

    fn component_id_or_init(_: &mut World) -> ComponentId {
        unsafe { std::mem::transmute(usize::MAX) }
    }

    fn component_id(_: &World) -> ComponentId {
        unsafe { std::mem::transmute(usize::MAX) }
    }
}

impl<C: Component> QueryItem for Option<&C> {
    type ItemRef<'a> = Option<&'a C>;
    type ItemMut<'a> = Option<&'a C>;
    #[inline]
    unsafe fn fetch_ref(world_ptr: WorldPtr<'_>, entity: Entity, component_index: ComponentId) -> Self::ItemRef<'_> {
        unsafe { world_ptr.as_world().get_component_by_id::<C>(entity, component_index) }
    }

    #[inline]
    unsafe fn fetch_mut(world_ptr: WorldPtr<'_>, entity: Entity, component_index: ComponentId) -> Self::ItemMut<'_> {
        unsafe { world_ptr.as_world().get_component_by_id::<C>(entity, component_index) }
    }

    fn component_id_or_init(world: &mut World) -> ComponentId {
        world.register_component::<C>()
    }
    
    #[inline]
    fn component_id(world: &World) -> ComponentId {
        world.get_component_id::<C>().expect("QueryItem component index not found")
    }

    #[inline]
    fn join_component_signature(component_signature: &mut HashSet<TypeId>) {
        component_signature.insert(TypeId::of::<C>());
    }

    #[inline]
    fn join_component_access(world: &mut World, component_access: &mut Access) {
        component_access.immutable.set(world.register_component::<C>().id())
    }
}

impl<C: Component> QueryItem for Option<&mut C> {
    type ItemRef<'a> = Option<&'a C>;
    type ItemMut<'a> = Option<&'a mut C>;
    #[inline]
    unsafe fn fetch_ref(world_ptr: WorldPtr<'_>, entity: Entity, component_index: ComponentId) -> Self::ItemRef<'_> {
        unsafe { world_ptr.as_world().get_component_by_id::<C>(entity, component_index) }
    }

    #[inline]
    unsafe fn fetch_mut(mut world_ptr: WorldPtr<'_>, entity: Entity, component_index: ComponentId) -> Self::ItemMut<'_> {
        unsafe { world_ptr.as_world_mut().get_mut_component_by_id::<C>(entity, component_index) }
    }

    fn component_id_or_init(world: &mut World) -> ComponentId {
        world.register_component::<C>()
    }
    
    #[inline]
    fn component_id(world: &World) -> ComponentId {
        world.get_component_id::<C>().expect("QueryItem component index not found")
    }

    #[inline]
    fn join_component_signature(component_signature: &mut HashSet<TypeId>) {
        component_signature.insert(TypeId::of::<C>());
    }

    #[inline]
    fn join_component_access(world: &mut World, component_access: &mut Access) {
        component_access.mutable.set(world.register_component::<C>().id());
        component_access.mutable_count += 1;
    }
}

pub trait QueryData: Sync + Send {
    type ItemRef<'a>;
    type ItemMut<'a>;
    unsafe fn fetch_ref<'a>(world_ptr: WorldPtr<'a>, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemRef<'a>;
    unsafe fn fetch_mut<'a>(world_ptr: WorldPtr<'a>, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemMut<'a>;
    fn join_component_signature(component_signature: &mut HashSet<TypeId>);
    fn join_required_component_signature(component_signature: &mut HashSet<TypeId>);
    fn join_component_access(world: &mut World, component_access: &mut Access);
    fn register_components(world: &mut World);
    fn cache_component_ids(world: &World) -> [ComponentId; QUERY_MAX_VARIADIC_COUNT];
}

macro_rules! query_tuple_impl {
    ($(($i:tt, $name:ident)),+) => {
        #[allow(unused_parens)]
        impl<$($name: QueryItem),+> QueryData for ($($name),+) {
            type ItemRef<'a> = ($($name::ItemRef<'a>),+);
            type ItemMut<'a> = ($($name::ItemMut<'a>),+);

            #[inline(always)]
            unsafe fn fetch_ref<'a>(world_ptr: WorldPtr<'a>, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemRef<'a> {
                unsafe { ($($name::fetch_ref(world_ptr, entity, component_indices[$i])),+) }
            }

            #[inline(always)]
            unsafe fn fetch_mut<'a>(world_ptr: WorldPtr<'a>, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemMut<'a> {
                unsafe { ($($name::fetch_mut(world_ptr, entity, component_indices[$i])),+) }
            }

            #[inline]
            fn join_component_signature(component_signature: &mut HashSet<TypeId>) {
                $($name::join_component_signature(component_signature);)+
            }

            #[inline]
            fn join_required_component_signature(component_signature: &mut HashSet<TypeId>) {
                $($name::join_required_component_signature(component_signature);)+
            }
            
            #[inline]
            fn join_component_access(world: &mut World, component_access: &mut Access) {
                $($name::join_component_access(world, component_access);)+
            }

            fn cache_component_ids(world: &World) -> [ComponentId; QUERY_MAX_VARIADIC_COUNT] {
                let mut cache = [MaybeUninit::uninit(); QUERY_MAX_VARIADIC_COUNT];
                $(cache[$i] = MaybeUninit::new($name::component_id(world));)+
                unsafe { std::mem::transmute::<_, [ComponentId; QUERY_MAX_VARIADIC_COUNT]>(cache) }
            }

            fn register_components(world: &mut World) {
                $($name::component_id_or_init(world);)+
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{query_tuple_impl, 1, 32, D}
