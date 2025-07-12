use crate::{access::Access, bitmap::Bitmap, component_manager::ComponentManager, Component, Entity};
use std::ptr::NonNull;

pub struct Query<D: QueryData> {
    _a: std::marker::PhantomData<D>,
    component_manager: SyncPointer<ComponentManager>,
}

struct SyncPointer<T>(NonNull<T>);
unsafe impl<T> Sync for SyncPointer<T> {}
unsafe impl<T> Send for SyncPointer<T> {}

impl<D: QueryData> Query<D> {
    pub fn new(component_manager: &ComponentManager) -> Self {
        Self {
            _a: std::marker::PhantomData,
            component_manager: SyncPointer(NonNull::from(component_manager)),
        }
    }

    pub fn iter(&self) -> QueryIter<D> {
        QueryIter {
            query: self,
            iter: unsafe { self.component_manager.0.as_ref().entity_component_signatures.iter() },
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
                    unsafe { D::fetch(self.query.component_manager.0.as_ref(), *entity) }
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
    fn component_access() -> Access;
}

impl<T: Component + 'static> QueryData for &T {
    type Item<'a> = &'a T;
    unsafe fn fetch(component_manager: &ComponentManager, entity: Entity) -> Self::Item<'_> {
        unsafe { component_manager.get_entity_component::<T>(entity).unwrap_unchecked().as_ref().unwrap_unchecked() }
    }

    fn signature() -> Bitmap {
        Bitmap::new().with_set(T::signature_index())
    }

    fn component_access() -> Access {
        Access {
            immutable: Bitmap::new().with_set(T::signature_index()),
            mutable: Bitmap::new(),
            mutable_count: 0,
        }
    }
}

impl<T: Component + 'static> QueryData for &mut T {
    type Item<'a> = &'a mut T;
    unsafe fn fetch(component_manager: &ComponentManager, entity: Entity) -> Self::Item<'_> {
        unsafe { component_manager.get_mut_entity_component::<T>(entity).unwrap_unchecked().as_mut().unwrap_unchecked() }
    }

    fn signature() -> Bitmap {
        Bitmap::default().with_set(T::signature_index())
    }

    fn component_access() -> Access {
        Access {
            immutable: Bitmap::new(),
            mutable: Bitmap::new().with_set(T::signature_index()),
            mutable_count: 1,
        }
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
            
            fn component_access() -> Access {
                Access {
                    immutable: $($name::component_access().immutable)|+,
                    mutable: $($name::component_access().mutable)|+,
                    mutable_count: $($name::component_access().mutable_count+)+0,
                }
            }
        }
    }
}

variadics_please::all_tuples!{query_tuple_impl, 2, 32, D}
