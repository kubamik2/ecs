use crate::{access::Access, component_manager::ComponentManager, query::QueryData, resource::ResourceManager, Query};

pub trait SystemParam where Self: Sized {
    fn create(component_manager: &ComponentManager, resource_manager: &ResourceManager) -> Option<Self>;
    fn component_access() -> Access {
        Access::default()
    }
    fn resource_access() -> Access {
        Access::default()
    }
}

impl<D: QueryData> SystemParam for Query<D> {
    fn create(component_manager: &ComponentManager, _: &ResourceManager) -> Option<Self> {
        Some(Query::new(component_manager))
    }

    fn component_access() -> Access {
        D::component_access()   
    }
}

pub trait SystemFunc<In> {
    fn run(&self, component_manager: &ComponentManager, resource_manager: &ResourceManager);
    fn component_access(&self) -> Access;
    fn resource_access(&self) -> Access;
}


macro_rules! system_func_impl {
    ($(($param:ident, $p:ident)),+) => {
        impl<F, $($param),+> SystemFunc<($($param),+)> for F where
            F: Send + Sync + 'static,
            for<'a> &'a F: FnMut($($param),+),
            $($param: SystemParam + 'static),+
        {
            fn run(&self, component_manager: &ComponentManager, resource_manager: &ResourceManager) {
                fn call<$($param),+>(mut f: impl FnMut($($param),+), $($p:$param),+) {
                    f($($p),+);
                }
                $(let $p = $param::create(component_manager, resource_manager);)+
                if $($p.is_some())&&+ {
                    unsafe {call(self, $($p.unwrap_unchecked()),+)};
                }
            }
            
            fn component_access(&self) -> Access {
                $(let $p = $param::component_access();)+
                Access {
                    immutable: $($p.immutable)|+,
                    mutable: $($p.mutable)|+,
                    mutable_count: $($p.mutable_count+)+0
                }
            }

            fn resource_access(&self) -> Access {
                $(let $p = $param::resource_access();)+
                Access {
                    immutable: $($p.immutable)|+,
                    mutable: $($p.mutable)|+,
                    mutable_count: $($p.mutable_count+)+0
                }
            }
        }
    }
}

variadics_please::all_tuples!{system_func_impl, 1, 32, In, p}
