use proc_macro::TokenStream;
use syn::DeriveInput;

#[proc_macro_derive(Component)]
pub fn component_derive_macro(item: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(item).unwrap();
    let ident = &ast.ident;
    let generics = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote::quote! {
        impl #impl_generics ecs::Component for #ident #ty_generics #where_clause {}
    }.into()
}

#[proc_macro_derive(Resource, attributes(link_resources))]
pub fn resource_derive_macro(item: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(item).unwrap();
    let ident = &ast.ident;
    let generics = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut linked_resources = Vec::new();
    if let Some(attr) = ast.attrs.iter().find(|p| p.path().is_ident("link_resources")) {
        let _ = attr.parse_nested_meta(|meta| {
            linked_resources.push(meta.path);
            Ok(())
        });
    }

    quote::quote! {
        impl #impl_generics ecs::Resource for #ident #ty_generics #where_clause {
            fn join_additional_resource_access<F: FnMut(ecs::ResourceId) -> Result<(), ecs::access::Conflict>>(world: &mut ecs::World, mut f: F) -> Result<(), ecs::param::SystemParamError> {
                #(f(world.get_resource_id::<#linked_resources>().ok_or(ecs::param::SystemParamError::MissingResource(std::any::type_name::<#linked_resources>()))?)?;)*
                Ok(())
            }
        }
    }.into()
}

#[proc_macro_derive(ScheduleLabel)]
pub fn schedule_label_derive_macro(item: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(item).unwrap();
    let ident = &ast.ident;
    let generics = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote::quote! {
        impl #impl_generics ecs::ScheduleLabel for #ident #ty_generics #where_clause {}
    }.into()
}
