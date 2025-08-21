use crate::param::ObserverParam;

pub trait ObserverInput {}

macro_rules! observer_input_impl {
    ($($param:ident),+) => {
        #[allow(unused_parens)]
        impl<$($param: ObserverParam),+> ObserverInput for ($($param),+) {}
    }
}

variadics_please::all_tuples!{observer_input_impl, 1, 32, In}
