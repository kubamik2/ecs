use crate::component_manager::Signature;
use super::{access::Access, bitmap::Bitmap, component_manager::ComponentManager, Component, EntityId, ECS};
use std::{any::TypeId, collections::HashSet, ptr::NonNull};

const QUERY_MAX_VARIADIC_COUNT: usize = 32;

pub struct Query<D: QueryData> {
    _a: std::marker::PhantomData<D>,
    ecs: SyncPointer<ECS>,
    component_signature: Signature,
    cached_component_indices: [Option<u32>; QUERY_MAX_VARIADIC_COUNT],
}

struct SyncPointer<T>(NonNull<T>);
unsafe impl<T> Sync for SyncPointer<T> {}
unsafe impl<T> Send for SyncPointer<T> {}

impl<D: QueryData> Query<D> {
    pub fn new(ecs: &ECS) -> Option<Self> {
        let mut component_signature = Bitmap::new();
        let mut component_signature_map = HashSet::new();
        D::join_required_component_signature(&mut component_signature_map);
        for type_id in component_signature_map {
            let signature = ecs.component_manager.get_component_signature(&type_id)?;
            component_signature |= signature;
        }
        Some(Self {
            _a: std::marker::PhantomData,
            ecs: SyncPointer(NonNull::from(ecs)),
            component_signature,
            cached_component_indices: D::cache_component_indices(&ecs.component_manager),
        })
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
            .map(|entity_id| unsafe { D::fetch_ref(&ecs.component_manager, entity_id, &self.cached_component_indices) })
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
            .map(|entity_id| unsafe { D::fetch_mut(&ecs.component_manager, entity_id, &self.cached_component_indices) })
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
            .map(|entity_id| unsafe { D::fetch_mut(&ecs.component_manager, entity_id, &self.cached_component_indices) })
    }

    pub fn get(&self, entity_id: EntityId) -> Option<D::ItemRef<'_>> {
        let ecs = unsafe { self.ecs.0.as_ref() };
        let entity_signature = ecs.component_manager.get_entity_component_signature(entity_id)?;
        if (entity_signature & self.component_signature) != self.component_signature {
            return None;
        }
        Some(unsafe { D::fetch_ref(&ecs.component_manager, entity_id, &self.cached_component_indices) })
    }

    pub fn get_mut(&mut self, entity_id: EntityId) -> Option<D::ItemMut<'_>> {
        let ecs = unsafe { self.ecs.0.as_ref() };
        let entity_signature = ecs.component_manager.get_entity_component_signature(entity_id)?;
        if (entity_signature & self.component_signature) != self.component_signature {
            return None;
        }
        Some(unsafe { D::fetch_mut(&ecs.component_manager, entity_id, &self.cached_component_indices) })
    }

    /// # Safety
    ///
    /// Might violate rust's reference rules
    pub unsafe fn get_unsafe(&self, entity_id: EntityId) -> Option<D::ItemMut<'_>> {
        let ecs = unsafe { self.ecs.0.as_ref() };
        let entity_signature = ecs.component_manager.get_entity_component_signature(entity_id)?;
        if (entity_signature & self.component_signature) != self.component_signature {
            return None;
        }
        Some(unsafe { D::fetch_mut(&ecs.component_manager, entity_id, &self.cached_component_indices) })
    }
}

pub trait QueryItem {
    type ItemRef<'a>;
    type ItemMut<'a>;
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemRef<'a>;
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemMut<'a>;
    fn component_index(_: &ComponentManager) -> Option<u32> { None }
    fn join_component_signature(_: &mut HashSet<TypeId>) {}
    fn join_component_access(_: &mut Access) {}
    fn join_required_component_signature(_: &mut HashSet<TypeId>) {}
}

impl<C: Component> QueryItem for &C {
    type ItemRef<'a> = &'a C;
    type ItemMut<'a> = &'a C;
    #[inline]
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemRef<'a> {
        unsafe { component_manager.get_component_ptr_by_index_unchecked::<C>(entity_id, component_index.unwrap_unchecked()).as_ref().unwrap_unchecked() }
    }

    #[inline]
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemMut<'a> {
        unsafe { component_manager.get_component_ptr_by_index_unchecked::<C>(entity_id, component_index.unwrap_unchecked()).as_ref().unwrap_unchecked() }
    }
    
    #[inline]
    fn component_index(component_manager: &ComponentManager) -> Option<u32> {
        Some(component_manager.get_component_index::<C>().expect("QueryItem component index not found"))
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
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemRef<'a> {
        unsafe { component_manager.get_component_ptr_by_index_unchecked::<C>(entity_id, component_index.unwrap_unchecked()).as_ref().unwrap_unchecked() }
    }

    #[inline]
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemMut<'a> {
        unsafe { component_manager.get_mut_component_ptr_by_index_unchecked::<C>(entity_id, component_index.unwrap_unchecked()).as_mut().unwrap_unchecked() }
    }
    
    #[inline]
    fn component_index(component_manager: &ComponentManager) -> Option<u32> {
        Some(component_manager.get_component_index::<C>().expect("QueryItem component index not found"))
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

impl QueryItem for EntityId {
    type ItemRef<'a> = EntityId;
    type ItemMut<'a> = EntityId;

    #[inline]
    unsafe fn fetch_ref<'a>(_: &ComponentManager, entity_id: EntityId, _: Option<u32>) -> Self::ItemRef<'a> {
        entity_id
    }

    #[inline]
    unsafe fn fetch_mut<'a>(_: &ComponentManager, entity_id: EntityId, _: Option<u32>) -> Self::ItemMut<'a> {
        entity_id
    }
}

impl<C: Component> QueryItem for Option<&C> {
    type ItemRef<'a> = Option<&'a C>;
    type ItemMut<'a> = Option<&'a C>;
    #[inline]
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemRef<'a> {
        unsafe { component_manager.get_component_ptr_by_index::<C>(entity_id, component_index.unwrap_unchecked()).map(|f| f.as_ref().unwrap_unchecked()) }
    }

    #[inline]
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemMut<'a> {
        unsafe { component_manager.get_component_ptr_by_index::<C>(entity_id, component_index.unwrap_unchecked()).map(|f| f.as_ref().unwrap_unchecked()) }
    }
    
    #[inline]
    fn component_index(component_manager: &ComponentManager) -> Option<u32> {
        Some(component_manager.get_component_index::<C>().expect("QueryItem component index not found"))
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
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemRef<'a> {
        unsafe { component_manager.get_component_ptr_by_index::<C>(entity_id, component_index.unwrap_unchecked()).map(|f| f.as_ref().unwrap_unchecked()) }
    }

    #[inline]
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_index: Option<u32>) -> Self::ItemMut<'a> {
        unsafe { component_manager.get_mut_component_ptr_by_index::<C>(entity_id, component_index.unwrap_unchecked()).map(|f| f.as_mut().unwrap_unchecked()) }
    }
    
    #[inline]
    fn component_index(component_manager: &ComponentManager) -> Option<u32> {
        Some(component_manager.get_component_index::<C>().expect("QueryItem component index not found"))
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

pub trait QueryData {
    type ItemRef<'a>;
    type ItemMut<'a>;
    unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_indices: &[Option<u32>; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemRef<'a>;
    unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_indices: &[Option<u32>; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemMut<'a>;
    fn join_component_signature(component_signature: &mut HashSet<TypeId>);
    fn join_required_component_signature(component_signature: &mut HashSet<TypeId>);
    fn join_component_access(component_access: &mut Access);
    fn cache_component_indices(component_manager: &ComponentManager) -> [Option<u32>; QUERY_MAX_VARIADIC_COUNT];
}

macro_rules! query_tuple_impl {
    ($(($i:tt, $name:ident)),+) => {
        #[allow(unused_parens)]
        impl<$($name: QueryItem),+> QueryData for ($($name),+) {
            type ItemRef<'a> = ($($name::ItemRef<'a>),+);
            type ItemMut<'a> = ($($name::ItemMut<'a>),+);

            #[inline(always)]
            unsafe fn fetch_ref<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_indices: &[Option<u32>; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemRef<'a> {
                unsafe { ($($name::fetch_ref(component_manager, entity_id, component_indices[$i])),+) }
            }

            #[inline(always)]
            unsafe fn fetch_mut<'a>(component_manager: &ComponentManager, entity_id: EntityId, component_indices: &[Option<u32>; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemMut<'a> {
                unsafe { ($($name::fetch_mut(component_manager, entity_id, component_indices[$i])),+) }
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

            fn cache_component_indices(component_manager: &ComponentManager) -> [Option<u32>; QUERY_MAX_VARIADIC_COUNT] {
                let mut cache = [None; QUERY_MAX_VARIADIC_COUNT];
                $(cache[$i] = $name::component_index(component_manager);)+
                cache
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{query_tuple_impl, 1, 32, D}
