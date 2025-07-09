use crate::{bitmap::Bitmap, component_manager::ComponentManager, Component, Entity};
use std::{cell::{Ref, RefMut}, ptr::NonNull};

pub struct Query<D: QueryData> {
    _a: std::marker::PhantomData<D>,
    component_manager: NonNull<ComponentManager>,
}

impl<D: QueryData> Query<D> {
    pub fn new(component_manager: &ComponentManager) -> Self {
        Self {
            _a: std::marker::PhantomData,
            component_manager: NonNull::from(component_manager),
        }
    }

    pub fn iter(&self) -> QueryIter<D> {
        QueryIter {
            query: self,
            iter: unsafe { self.component_manager.as_ref().entity_component_signatures.iter() },
        }
    }
}

pub struct QueryIter<'a, D: QueryData> {
    query: &'a Query<D>,
    iter: std::collections::hash_map::Iter<'a, Entity, Bitmap>,
}

impl<'a, D: QueryData> Iterator for QueryIter<'a, D> {
    type Item = (Entity, D::Item<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        for (entity, bitmap) in self.iter.by_ref() {
            if (*bitmap & D::signature()) == D::signature() {
                return Some((
                    *entity,
                    unsafe { D::fetch(self.query.component_manager.as_ref(), *entity) }
                ))
            }
        }
        None
    }
}

pub trait QueryData {
    type Item<'a>;
    /// .
    /// # Safety
    /// Must be called on entity with appropriate components
    /// .
    unsafe fn fetch(component_manager: &ComponentManager, entity: Entity) -> Self::Item<'_>;
    fn signature() -> Bitmap;
}

impl<T: Component + 'static> QueryData for &T {
    type Item<'a> = Ref<'a, T>;
    unsafe fn fetch(component_manager: &ComponentManager, entity: Entity) -> Self::Item<'_> {
        unsafe { component_manager.get_entity_component(entity).unwrap_unchecked() }
    }

    fn signature() -> Bitmap {
        Bitmap::default().with_set(T::signature_index())
    }
}

impl<T: Component + 'static> QueryData for &mut T {
    type Item<'a> = RefMut<'a, T>;
    unsafe fn fetch(component_manager: &ComponentManager, entity: Entity) -> Self::Item<'_> {
        unsafe { component_manager.get_mut_entity_component(entity).unwrap_unchecked() }
    }

    fn signature() -> Bitmap {
        Bitmap::default().with_set(T::signature_index())
    }
}

macro_rules! query_tuple_impl {
    ($($name:ident),+) => {
        impl<$($name: QueryData),+> QueryData for ($($name),+) {
            type Item<'a> = ($($name::Item<'a>),+);
            unsafe fn fetch(component_manager: &ComponentManager, entity: Entity) -> Self::Item<'_> {
                unsafe { ($($name::fetch(component_manager, entity)),+) }
            }

            fn signature() -> Bitmap {
                $($name::signature())|+
            }
        }
    }
}

variadics_please::all_tuples!{query_tuple_impl, 2, 32, D}
