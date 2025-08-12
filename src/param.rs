use super::{access::Access, query::QueryData, Query, ECS};

#[allow(unused_variables)]
pub trait SystemParam where Self: Sized {
    fn create(ecs: &ECS) -> Option<Self>;
    fn join_component_access(component_access: &mut Access) {}
    fn join_resource_access(resource_access: &mut Access) {}
}

impl<D: QueryData> SystemParam for Query<D> {
    fn create(ecs: &ECS) -> Option<Self> {
        Query::new(ecs)
    }

    fn join_component_access(component_access: &mut Access) {
        D::join_component_access(component_access);
    }
}

pub trait SystemFunc<In> {
    fn run(&self, ecs: &ECS);
    fn join_component_access(component_access: &mut Access);
    fn join_resource_access(resource_access: &mut Access);
}

impl<F> SystemFunc<()> for F where 
    F: Send + Sync + 'static,
    for<'a> &'a F: FnMut()
{
    fn run(&self, _: &ECS) {
        fn call(mut f: impl FnMut()) {
            f()
        }
        call(self)
    }

    fn join_resource_access(_: &mut Access) {}

    fn join_component_access(_: &mut Access) {}
}


macro_rules! system_func_impl {
    ($(($param:ident, $p:ident)),+) => {
        #[allow(unused_parens)]
        impl<F, $($param),+> SystemFunc<($($param),+)> for F where
            F: Send + Sync + 'static,
            for<'a> &'a F: FnMut($($param),+),
            $($param: SystemParam + 'static),+
        {
            fn run(&self, ecs: &ECS) {
                #[allow(clippy::too_many_arguments)]
                fn call<$($param),+>(mut f: impl FnMut($($param),+), $($p:$param),+) {
                    f($($p),+);
                }
                $(let $p = $param::create(ecs);)+
                if $($p.is_some())&&+ {
                    unsafe {call(self, $($p.unwrap_unchecked()),+)};
                }
            }
            
            fn join_component_access(component_access: &mut Access) {
                $($param::join_component_access(component_access);)+
            }

            fn join_resource_access(resource_access: &mut Access) {
                $($param::join_resource_access(resource_access);)+
            }
        }
    }
}

variadics_please::all_tuples!{system_func_impl, 1, 32, In, p}
