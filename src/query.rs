use crate::{ComponentBundle, ComponentId, Signature, access::{AccessBuilder, Conflict, FilteredComponentAccess}, bitmap::Bitmap, param::{SystemParam, SystemParamError}, system::SystemHandle, world::WorldPtr};
use super::{access::Access, Component, Entity, World};
use std::{any::TypeId, collections::HashSet, marker::PhantomData, mem::MaybeUninit, ops::Deref};

const QUERY_MAX_VARIADIC_COUNT: usize = 32;

pub struct Query<'a, D: QueryData, F: QueryFilter = ()> {
    _a: std::marker::PhantomData<(D, F)>,
    world_ptr: WorldPtr<'a>,
    required: Bitmap,
    forbidden: Bitmap,
    cached_component_ids: [ComponentId; QUERY_MAX_VARIADIC_COUNT],
}

impl<'a, D: QueryData, F: QueryFilter> Query<'a, D, F> {
    pub fn new(world: &'a mut World) -> Result<Self, Conflict> {
        let mut access = FilteredComponentAccess::default();
        D::join_filtered_component_access(world, &mut access)?;
        F::join_filtered_component_access(world, &mut access)?;
        let required = *access.immutable() | *access.mutable() | *access.with();
        let forbidden = *access.without();
        let cached_component_ids = D::cache_component_ids(world);
        Ok(Self {
            _a: std::marker::PhantomData,
            world_ptr: world.world_ptr_mut(),
            required,
            forbidden,
            cached_component_ids,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = D::ItemRef<'a>> {
        let required_signature = self.required;
        let forbidden_signature = self.forbidden;
        let world_ptr = self.world_ptr;
        unsafe { world_ptr.as_world() }
            .groups()
            .iter()
            .filter(move |(signature, _)| {
                let signature = **signature;
                (signature & required_signature == required_signature) &&
                (signature & forbidden_signature).is_zero()
            })
            .flat_map(|(_, entities)| entities.iter().copied())
            .map(|entity| unsafe { D::fetch_ref(self.world_ptr, entity, &self.cached_component_ids) })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = D::ItemMut<'a>> {
        let required_signature = self.required;
        let forbidden_signature = self.forbidden;
        unsafe { self.world_ptr.as_world() }
            .groups()
            .iter()
            .filter(move |(signature, _)| {
                let signature = **signature;
                (signature & required_signature == required_signature) &&
                (signature & forbidden_signature).is_zero()
            })
            .flat_map(|(_, entities)| entities.iter().copied())
            .map(|entity| unsafe { D::fetch_mut(self.world_ptr, entity, &self.cached_component_ids) })
    }

    /// # Safety
    /// can violate rust's reference rules
    pub unsafe fn iter_unsafe(&self) -> impl Iterator<Item = D::ItemMut<'a>> {
        let required_signature = self.required;
        let forbidden_signature = self.forbidden;
        unsafe { self.world_ptr.as_world() }
            .groups()
            .iter()
            .filter(move |(signature, _)| {
                let signature = **signature;
                (signature & required_signature == required_signature) &&
                (signature & forbidden_signature).is_zero()
            })
            .flat_map(|(_, entities)| entities.iter().copied())
            .map(|entity| unsafe { D::fetch_mut(self.world_ptr, entity, &self.cached_component_ids) })
    }

    pub fn get(&self, entity: Entity) -> Option<D::ItemRef<'_>> {
        let entity_signature = unsafe { self.world_ptr.as_world() }.get_entity_signature(entity)?;
        if ((entity_signature & self.required) != self.required) ||
        !(entity_signature & self.forbidden).is_zero()
        {
            return None;
        }
        Some(unsafe { D::fetch_ref(self.world_ptr, entity, &self.cached_component_ids) })
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<D::ItemMut<'_>> {
        let entity_signature = unsafe { self.world_ptr.as_world() }.get_entity_signature(entity)?;
        if ((entity_signature & self.required) != self.required) ||
        !(entity_signature & self.forbidden).is_zero()
        {
            return None;
        }
        Some(unsafe { D::fetch_mut(self.world_ptr, entity, &self.cached_component_ids) })
    }

    /// # Safety
    ///
    /// Might violate rust's reference rules
    pub unsafe fn get_unsafe(&self, entity: Entity) -> Option<D::ItemMut<'_>> {
        let entity_signature = unsafe { self.world_ptr.as_world() }.get_entity_signature(entity)?;
        if ((entity_signature & self.required) != self.required) ||
        !(entity_signature & self.forbidden).is_zero()
        {
            return None;
        }
        Some(unsafe { D::fetch_mut(self.world_ptr, entity, &self.cached_component_ids) })
    }

    // for testing purposes
    pub(crate) fn required(&self) -> &Bitmap {
        &self.required
    }

    // for testing purposes
    pub(crate) fn forbidden(&self) -> &Bitmap {
        &self.forbidden
    }

    // for testing purposes
    pub(crate) fn filtered_component_access(world: &mut World) -> Result<FilteredComponentAccess, Conflict> {
        let mut access = FilteredComponentAccess::default();
        D::join_filtered_component_access(world, &mut access)?;
        F::join_filtered_component_access(world, &mut access)?;
        Ok(access)
    }
}

unsafe impl<D: QueryData, F: QueryFilter> SystemParam for Query<'_, D, F> {
    type Item<'a> = Query<'a, D, F>;
    type State = (Signature, [ComponentId; QUERY_MAX_VARIADIC_COUNT], Signature);

    fn join_access(world: &mut World, access: &mut AccessBuilder) -> Result<(), SystemParamError> {
        let mut filtered_component_access = FilteredComponentAccess::default();
        D::join_filtered_component_access(world, &mut filtered_component_access).map_err(SystemParamError::Conflict)?;
        F::join_filtered_component_access(world, &mut filtered_component_access).map_err(SystemParamError::Conflict)?;
        access.join_filtered_component_access(filtered_component_access).map_err(SystemParamError::Conflict)
    }

    fn init_state(world: &mut World, _: &SystemHandle) -> Result<Self::State, SystemParamError> {
        let query = Query::<D, F>::new(world).map_err(SystemParamError::Conflict)?;
        Ok((query.required, query.cached_component_ids, query.forbidden))
    }

    unsafe fn fetch<'a>(world_ptr: WorldPtr<'a>, state: &'a mut Self::State, _: &SystemHandle) -> Self::Item<'a> {
        Query {
            _a: Default::default(),
            cached_component_ids: state.1,
            required: state.0,
            forbidden: state.2,
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
    fn join_filtered_component_access(_: &mut World, _: &mut FilteredComponentAccess) -> Result<(), Conflict> { Ok(()) }
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

    fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict> {
        access.add_immutable(world.register_component::<C>().get())
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
        unsafe { world_ptr.as_world_mut().get_component_by_id_unchecked_mut::<C>(entity, component_index) }
    }

    fn component_id_or_init(world: &mut World) -> ComponentId {
        world.register_component::<C>()
    }
    
    #[inline]
    fn component_id(world: &World) -> ComponentId {
        world.get_component_id::<C>().expect("QueryItem component index not found")
    }

    fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict> {
        access.add_mutable(world.register_component::<C>().get())
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
    fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict> {
        access.add_immutable(world.register_component::<C>().get())
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
        unsafe { world_ptr.as_world_mut().get_component_by_id_mut::<C>(entity, component_index) }
    }

    fn component_id_or_init(world: &mut World) -> ComponentId {
        world.register_component::<C>()
    }
    
    #[inline]
    fn component_id(world: &World) -> ComponentId {
        world.get_component_id::<C>().expect("QueryItem component index not found")
    }

    #[inline]
    fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict> {
        access.add_mutable(world.register_component::<C>().get())
    }
}

pub struct Children<'a>(&'a [Entity]);

impl<'a> Deref for Children<'a> {
    type Target = [Entity];
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl QueryItem for Children<'_> {
    type ItemRef<'a> = Children<'a>;
    type ItemMut<'a> = Children<'a>;

    #[inline]
    unsafe fn fetch_ref(world_ptr: WorldPtr<'_>, entity: Entity, _: ComponentId) -> Self::ItemRef<'_> {
        Children(unsafe { world_ptr.as_world() }.children(entity))
    }

    #[inline]
    unsafe fn fetch_mut(world_ptr: WorldPtr<'_>, entity: Entity, _: ComponentId) -> Self::ItemMut<'_> {
        Children(unsafe { world_ptr.as_world() }.children(entity))
    }

    fn component_id_or_init(_: &mut World) -> ComponentId {
        unsafe { std::mem::transmute(usize::MAX) }
    }

    fn component_id(_: &World) -> ComponentId {
        unsafe { std::mem::transmute(usize::MAX) }
    }
}

pub trait QueryData: Sync + Send {
    type ItemRef<'a>;
    type ItemMut<'a>;
    unsafe fn fetch_ref<'a>(world_ptr: WorldPtr<'a>, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemRef<'a>;
    unsafe fn fetch_mut<'a>(world_ptr: WorldPtr<'a>, entity: Entity, component_indices: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemMut<'a>;
    fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict>;
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
            fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict> {
                $($name::join_filtered_component_access(world, access)?;)+
                Ok(())
            }

            fn cache_component_ids(world: &World) -> [ComponentId; QUERY_MAX_VARIADIC_COUNT] {
                let mut cache = [MaybeUninit::uninit(); QUERY_MAX_VARIADIC_COUNT];
                $(cache[$i] = MaybeUninit::new($name::component_id(world));)+
                unsafe { std::mem::transmute::<_, [ComponentId; QUERY_MAX_VARIADIC_COUNT]>(cache) }
            }
        }
    }
}

variadics_please::all_tuples_enumerated!{query_tuple_impl, 1, 32, D}

impl QueryData for () {
    type ItemRef<'a> = ();
    type ItemMut<'a> = ();
    fn cache_component_ids(_: &World) -> [ComponentId; QUERY_MAX_VARIADIC_COUNT] {
        std::array::from_fn(|_| unsafe { std::mem::transmute(usize::MAX) })
    }
    fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict> { Ok(()) }
    unsafe fn fetch_ref<'a>(_: WorldPtr<'a>, _: Entity, _: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemRef<'a> {}
    unsafe fn fetch_mut<'a>(_: WorldPtr<'a>, _: Entity, _: &[ComponentId; QUERY_MAX_VARIADIC_COUNT]) -> Self::ItemMut<'a> {}
}

pub trait QueryFilter {
    fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict>;
}

macro_rules! query_filter_impl {
    ($($name:ident),+) => {
        impl<$($name: QueryFilter),+> QueryFilter for ($($name),+) {
            fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict> {
                $($name::join_filtered_component_access(world, access)?;)+
                Ok(())
            }
        }
    }
}

impl QueryFilter for () {
    fn join_filtered_component_access(_: &mut World, _: &mut FilteredComponentAccess) -> Result<(), Conflict> { Ok(()) }
}

variadics_please::all_tuples!{query_filter_impl, 2, 32, C}

pub struct With<B: ComponentBundle + 'static>(PhantomData<B>);
pub struct Without<B: ComponentBundle + 'static>(PhantomData<B>);

impl<B: ComponentBundle + 'static> QueryFilter for With<B> {
    fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict> {
        access.join_with(B::signature(world))
    }
}

impl<B: ComponentBundle + 'static> QueryFilter for Without<B> {
    fn join_filtered_component_access(world: &mut World, access: &mut FilteredComponentAccess) -> Result<(), Conflict> {
        access.join_without(B::signature(world))
    }
}
