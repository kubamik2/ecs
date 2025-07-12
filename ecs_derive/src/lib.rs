use proc_macro::TokenStream;
use syn::DeriveInput;
use std::sync::atomic::{AtomicUsize, Ordering};

static COMPONENTS: AtomicUsize = AtomicUsize::new(0);
const MAX_COMPONENTS: usize = 32;

static RESOURCES: AtomicUsize = AtomicUsize::new(0);
const MAX_RESOURCES: usize = 32;

#[proc_macro_derive(Component)]
pub fn component_derive_macro(item: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(item).unwrap();
    let ident = ast.ident;
    let id = COMPONENTS.fetch_add(1, Ordering::Relaxed);
    assert!(id < MAX_COMPONENTS);

    quote::quote! {
        impl ecs::Component for #ident {
            fn signature_index() -> usize {
                #id
            }
        }
    }.into()
}

#[proc_macro_derive(Resource)]
pub fn resource_derive_macro(item: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(item).unwrap();
    let ident = ast.ident;
    let id = RESOURCES.fetch_add(1, Ordering::Relaxed);
    assert!(id < MAX_RESOURCES);

    quote::quote! {
        impl ecs::Resource for #ident {
            fn signature_index() -> usize {
                #id
            }
        }
    }.into()
}
