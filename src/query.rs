use crate::{component_manager::{ComponentId, Signature}, param::SystemParam};
use super::{access::Access, bitmap::Bitmap, component_manager::ComponentManager, Component, Entity, ECS};
use std::{any::TypeId, collections::HashSet, mem::MaybeUninit, ptr::NonNull};

const QUERY_MAX_VARIADIC_COUNT: usize = 32;

pub struct Query<D: QueryData> {
    _a: std::marker::PhantomData<D>,
    ecs: SyncPointer<ECS>,
    component_signature: Signature,
    cached_component_ids: [ComponentId; QUERY_MAX_VARIADIC_COUNT],
}

impl<D: QueryData> Clone for Query<D> {
    fn clone(&self) -> Self {
        Self {
            _a: Default::default(),
            ecs: self.ecs.clone(),
            component_signature: self.component_signature,
            cached_component_ids: self.cached_component_ids,
        }
    }
}

struct SyncPointer<T>(NonNull<T>);
unsafe impl<T> Sync for SyncPointer<T> {}
unsafe impl<T> Send for SyncPointer<T> {}

impl<T> Clone for SyncPointer<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<D: QueryData> Query<D> {
    pub fn new(ecs: &mut ECS) -> Self {
        D::register_components(ecs);
        let mut component_signature = Bitmap::new();
        let mut component_signature_map = HashSet::new();
        D::join_required_component_signature(&mut component_signature_map);
        for type_id in component_signature_map {
            let signature = ecs.component_manager.get_component_signature(&type_id).expect("Query::new component not registered");
            component_signature |= signature;
        }
        let cached_component_ids = D::cache_component_ids(&ecs.component_manager);
        Self {
            _a: std::marker::PhantomData,
            ecs: SyncPointer(NonNull::from(ecs)),
            component_signature,
            cached_component_ids,
        }
    }

    pub fn iter<'a>(&self) -> impl Iterator<Item = D::ItemRef<'a>> {
        let ecs = unsafe { self.ecs.0.as_ref() };
        let queue_signature = self.component_signature;
        ecs
            .component_manager
            .groups()
            .iter()
            .filter(move |(signature, _)| {
                let signature = **signature;
                signature & queue_signature == queue_signature
            })
            .flat_map(|(_, entities)| entities.iter().copied())
            .map(|entity| unsafe { D::fetch_ref(&ecs.component_manager, entity, &self.cached_component_ids) })
    }

    pub fn iter_mut<'a>(&mut self) -> impl Iterator<Item = D::ItemMut<'a>> {
        let ecs = unsafe { self.ecs.0.as_ref() };
        let queue_signature = self.component_signature;
        ecs
            .component_manager
            .groups()
            .iter()
            .filter(move |(signature, _)| {
                let signature = **signature;
                signature & queue_signature == queue_signature
            })
            .flat_map(|(_, entities)| entities.iter().copied())
            .map(|entity| unsafe { D::fetch_mut(&ecs.component_manager, entity, &self.cached_component_ids) })
    }

    /// # Safety
    ///
    /// Might violate rust's reference rules
    pub unsafe fn iter_unsafe<'a>(&self) -> impl Iterator<Item = D::ItemMut<'a>> {
        let ecs = unsafe { self.ecs.0.as_ref() };
        let queue_signature = self.component_signature;
        ecs
            .component_manager
            .groups()
            .iter()
            .filter(move |(signature, _)| {
                let signature = **signature;
                signature & queue_signature == queue_signature
            })
            .flat_map(|(_, entities)| entities.iter().copied())
            .map(|entity| unsafe { D::fetch_mut(&ecs.component_manager, entity, &self.cached_component_ids) })
    }

    pub fn get(&self, entity: Entity) -> Option<D::ItemRef<'_>> {
        let ecs = unsafe { self.ecs.0.as_ref() };
        let entity_signature = ecs.component_manager.get_entity_component_signature(entity)?;
        if (entity_signature & self.component_signature) != self.component_signature {
            return None;
        }
        Some(unsafe { D::fetch_ref(&ecs.component_manager, entity, &self.cached_component_ids) })
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<D::ItemMut<'_>> {
        let ecs = unsafe { self.ecs.0.as_ref() };
        let entity_signature = ecs.component_manager.get_entity_component_signature(entity)?;
        if (entity_signature & self.component_signature) != self.component_signature {
            return None;
        }
        Some(unsafe { D::fetch_mut(&ecs.component_manager, entity, &self.cached_component_ids) })
    }

    /// # Safety
    ///
    /// Might violate rust's reference rules
    pub unsafe fn get_unsafe(&self, entity: Entity) -> Option<D::ItemMut<'_>> {
        let ecs = unsafe { self.ecs.0.as_ref() };
        let entity_signature = ecs.component_manager.get_entity_component_signature(entity)?;
        if (entity_signature & self.component_signature) != self.component_signature {
            return None;
        }
        Some(unsafe { D::fetch_mut(&ecs.component_manager, entity, &self.cached_component_ids) })
    }
}

impl<D: QueryData> SystemParam for Query<D> {
    type Item<'a> = Self;
    type State = Self;

    fn join_component_access(component_access: &mut Access) {
        D::join_component_access(component_access);
    }

    fn init_state(ecs: &mut ECS) -> Self::State {
        Query::new(ecs)
    }

    fn fetch<'a>(_: &'a ECS, state: &'a mut Self::State) -> Self::Item<'a> {
        state.clone()
    }
}

pub trait QueryItem: Send + Sync {
    type ItemRef<'a>;
    type ItemMut<'a>;
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity: Entity, component_index: ComponentId) -> Self::ItemRef<'a>;
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity: Entity, component_index: ComponentId) -> Self::ItemMut<'a>;
    fn component_id_or_init(ecs: &mut ECS) -> ComponentId;
    fn component_id(_: &ComponentManager) -> ComponentId;
    fn join_component_signature(_: &mut HashSet<TypeId>) {}
    fn join_component_access(_: &mut Access) {}
    fn join_required_component_signature(_: &mut HashSet<TypeId>) {}
}

impl<C: Component> QueryItem for &C {
    type ItemRef<'a> = &'a C;
    type ItemMut<'a> = &'a C;
    #[inline]
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity: Entity, component_id: ComponentId) -> Self::ItemRef<'a> {
        unsafe { component_manager.get_component_ptr_by_index_unchecked::<C>(entity, component_id).as_ref().unwrap_unchecked() }
    }

    #[inline]
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity: Entity, component_id: ComponentId) -> Self::ItemMut<'a> {
        unsafe { component_manager.get_component_ptr_by_index_unchecked::<C>(entity, component_id).as_ref().unwrap_unchecked() }
    }

    fn component_id_or_init(ecs: &mut ECS) -> ComponentId {
        ecs.register_component::<C>()
    }
    
    #[inline]
    fn component_id(component_manager: &ComponentManager) -> ComponentId {
        component_manager.get_component_id::<C>().expect("QueryItem component index not found")
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
    fn join_component_access(component_access: &mut Access) {
        component_access.immutable.insert(TypeId::of::<C>());
    }
}

impl<C: Component> QueryItem for &mut C {
    type ItemRef<'a> = &'a C;
    type ItemMut<'a> = &'a mut C;
    #[inline]
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity: Entity, component_index: ComponentId) -> Self::ItemRef<'a> {
        unsafe { component_manager.get_component_ptr_by_index_unchecked::<C>(entity, component_index).as_ref().unwrap_unchecked() }
    }

    #[inline]
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity: Entity, component_index: ComponentId) -> Self::ItemMut<'a> {
        unsafe { component_manager.get_mut_component_ptr_by_index_unchecked::<C>(entity, component_index).as_mut().unwrap_unchecked() }
    }

    fn component_id_or_init(ecs: &mut ECS) -> ComponentId {
        ecs.register_component::<C>()
    }
    
    #[inline]
    fn component_id(component_manager: &ComponentManager) -> ComponentId {
        component_manager.get_component_id::<C>().expect("QueryItem component index not found")
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
    fn join_component_access(component_access: &mut Access) {
        component_access.mutable.insert(TypeId::of::<C>());
        component_access.mutable_count += 1;
    }
}

impl QueryItem for Entity {
    type ItemRef<'a> = Entity;
    type ItemMut<'a> = Entity;

    #[inline]
    unsafe fn fetch_ref<'a>(_: &ComponentManager, entity: Entity, _: ComponentId) -> Self::ItemRef<'a> {
        entity
    }

    #[inline]
    unsafe fn fetch_mut<'a>(_: &ComponentManager, entity: Entity, _: ComponentId) -> Self::ItemMut<'a> {
        entity
    }

    fn component_id_or_init(_: &mut ECS) -> ComponentId {
        unsafe { std::mem::transmute(usize::MAX) }
    }

    fn component_id(_: &ComponentManager) -> ComponentId {
        unsafe { std::mem::transmute(usize::MAX) }
    }
}

impl<C: Component> QueryItem for Option<&C> {
    type ItemRef<'a> = Option<&'a C>;
    type ItemMut<'a> = Option<&'a C>;
    #[inline]
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity: Entity, component_index: ComponentId) -> Self::ItemRef<'a> {
        unsafe { component_manager.get_component_ptr_by_index::<C>(entity, component_index).map(|f| f.as_ref().unwrap_unchecked()) }
    }

    #[inline]
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity: Entity, component_index: ComponentId) -> Self::ItemMut<'a> {
        unsafe { component_manager.get_component_ptr_by_index::<C>(entity, component_index).map(|f| f.as_ref().unwrap_unchecked()) }
    }

    fn component_id_or_init(ecs: &mut ECS) -> ComponentId {
        ecs.register_component::<C>()
    }
    
    #[inline]
    fn component_id(component_manager: &ComponentManager) -> ComponentId {
        component_manager.get_component_id::<C>().expect("QueryItem component index not found")
    }

    #[inline]
    fn join_component_signature(component_signature: &mut HashSet<TypeId>) {
        component_signature.insert(TypeId::of::<C>());
    }

    #[inline]
    fn join_component_access(component_access: &mut Access) {
        component_access.immutable.insert(TypeId::of::<C>());
    }
}

impl<C: Component> QueryItem for Option<&mut C> {
    type ItemRef<'a> = Option<&'a C>;
    type ItemMut<'a> = Option<&'a mut C>;
    #[inline]
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity: Entity, component_index: ComponentId) -> Self::ItemRef<'a> {
        unsafe { component_manager.get_component_ptr_by_index::<C>(entity, component_index).map(|f| f.as_ref().unwrap_unchecked()) }
    }

    #[inline]
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity: Entity, component_index: ComponentId) -> Self::ItemMut<'a> {
        unsafe { component_manager.get_mut_component_ptr_by_index::<C>(entity, component_index).map(|f| f.as_mut().unwrap_unchecked()) }
    }

    fn component_id_or_init(ecs: &mut ECS) -> ComponentId {
        ecs.register_component::<C>()
    }
    
    #[inline]
    fn component_id(component_manager: &ComponentManager) -> ComponentId {
        component_manager.get_component_id::<C>().expect("QueryItem component index not found")
    }

    #[inline]
    fn join_component_signature(component_signature: &mut HashSet<TypeId>) {
        component_signature.insert(TypeId::of::<C>());
    }

    #[inline]
    fn join_component_access(component_access: &mut Access) {
        component_access.immutable.insert(TypeId::of::<C>());
    }
}

pub trait QueryData: Sync + Send {
    type ItemRef<'a>;
    type ItemMut<'a>;
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemRef<'a>;
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemMut<'a>;
    fn join_component_signature(component_signature: &mut HashSet<TypeId>);
    fn join_required_component_signature(component_signature: &mut HashSet<TypeId>);
    fn join_component_access(component_access: &mut Access);
    fn register_components(ecs: &mut ECS);
    fn cache_component_ids(component_manager: &ComponentManager) -> [ComponentId; QUERY_MAX_VARIADIC_COUNT];
}

macro_rules! query_tuple_impl {
    ($(($i:tt, $name:ident)),+) => {
        #[allow(unused_parens)]
        impl<$($name: QueryItem),+> QueryData for ($($name),+) {
            type ItemRef<'a> = ($($name::ItemRef<'a>),+);
            type ItemMut<'a> = ($($name::ItemMut<'a>),+);

            #[inline(always)]
            unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemRef<'a> {
                unsafe { ($($name::fetch_ref(component_manager, entity, component_indices[$i])),+) }
            }

            #[inline(always)]
            unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemMut<'a> {
                unsafe { ($($name::fetch_mut(component_manager, entity, component_indices[$i])),+) }
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
            fn join_component_access(component_access: &mut Access) {
                $($name::join_component_access(component_access);)+
            }

            fn cache_component_ids(component_manager: &ComponentManager) -> [ComponentId; QUERY_MAX_VARIADIC_COUNT] {
                let mut cache = [MaybeUninit::uninit(); QUERY_MAX_VARIADIC_COUNT];
                $(cache[$i] = MaybeUninit::new($name::component_id(component_manager));)+
                unsafe { std::mem::transmute::<_, [ComponentId; QUERY_MAX_VARIADIC_COUNT]>(cache) }
            }

            fn register_components(ecs: &mut ECS) {
                $($name::component_id_or_init(ecs);)+
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{query_tuple_impl, 1, 32, D}
