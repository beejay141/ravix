use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Fields, ItemStruct, Meta};

/// Parsed `#[inject(…)]` options.
enum InjectOptions {
    /// Plain `#[inject]` — required, resolves via `container.resolve()`.
    Required,
    /// `#[inject(optional)]` — field type must be `Option<…>`, resolves via
    /// `container.try_resolve()`.
    Optional,
    /// `#[inject(name = "…")]` — resolves via `container.resolve_named("…")`.
    Named(String),
    /// `#[inject(optional, name = "…")]` — resolves via
    /// `container.try_resolve_named("…")`.
    OptionalNamed(String),
}

fn parse_inject_options(attrs: &[syn::Attribute]) -> Option<InjectOptions> {
    for attr in attrs {
        let is_inject = attr
            .path()
            .get_ident()
            .map(|i| i == "inject")
            .unwrap_or(false);
        if !is_inject {
            continue;
        }

        // No args → plain required #[inject]
        let list = match &attr.meta {
            Meta::List(list) => list,
            Meta::Path(_) => return Some(InjectOptions::Required),
            _ => continue,
        };

        return Some(parse_inject_parens(&list.tokens));
    }
    None
}

fn parse_inject_parens(tokens: &proc_macro2::TokenStream) -> InjectOptions {
    // Simple parser: split on commas
    let s = tokens.to_string();
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();

    let mut optional = false;
    let mut name: Option<String> = None;

    for part in &parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part == "optional" {
            optional = true;
        } else if let Some(val) = part.strip_prefix("name = \"") {
            let val = val.strip_suffix('\"').unwrap_or(val);
            name = Some(val.to_string());
        } else if let Some(val) = part.strip_prefix("name=\"") {
            let val = val.strip_suffix('\"').unwrap_or(val);
            name = Some(val.to_string());
        }
    }

    match (optional, name) {
        (false, None) => InjectOptions::Required,
        (true, None) => InjectOptions::Optional,
        (false, Some(n)) => InjectOptions::Named(n),
        (true, Some(n)) => InjectOptions::OptionalNamed(n),
    }
}

/// Extract the inner type from `Option<Arc<dyn Trait>>` or `Option<Arc<ConcreteType>>`.
/// Returns the inner type if the field is `Option<...>`.
fn option_inner_type(ty: &syn::Type) -> Option<&syn::Type> {
    if let syn::Type::Path(type_path) = ty {
        let segs = &type_path.path.segments;
        if segs.len() == 1 {
            let seg = &segs[0];
            if seg.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if args.args.len() == 1 {
                        if let syn::GenericArgument::Type(inner) = &args.args[0] {
                            return Some(inner);
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn expand(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_struct = parse_macro_input!(input as ItemStruct);
    let struct_name = &input_struct.ident;

    let mut binding_check_submissions: Vec<TokenStream2> = Vec::new();

    let field_assignments: Vec<TokenStream2> = match &input_struct.fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| {
                let field_name = f.ident.as_ref().unwrap();
                let field_ty = &f.ty;

                let opts = match parse_inject_options(&f.attrs) {
                    Some(o) => o,
                    None => {
                        return quote! { #field_name: ::std::default::Default::default() };
                    }
                };

                match opts {
                    InjectOptions::Required => {
                        let type_name_str = quote!(#field_ty).to_string();
                        let field_ty2 = field_ty.clone();
                        binding_check_submissions.push(quote! {
                            ::ravix::inventory::submit!(::ravix::BindingCheck {
                                type_name: #type_name_str,
                                check: |c: &::ravix::Container| {
                                    if c.has_binding::<#field_ty2>() {
                                        ::std::result::Result::Ok(())
                                    } else {
                                        ::std::result::Result::Err(
                                            ::std::format!(
                                                "[ravix] Required binding missing for `{}`",
                                                #type_name_str,
                                            )
                                        )
                                    }
                                },
                            });
                        });
                        quote! { #field_name: container.resolve::<#field_ty>(), }
                    }
                    InjectOptions::Optional => {
                        let inner = option_inner_type(field_ty).unwrap_or_else(|| {
                            panic!(
                                "#[inject(optional)] requires the field type to be \
                                 Option<...>, but `{}` is not Option",
                                quote!(#field_ty)
                            )
                        });
                        quote! { #field_name: container.try_resolve::<#inner>(), }
                    }
                    InjectOptions::Named(n) => {
                        quote! { #field_name: container.resolve_named::<#field_ty>(#n), }
                    }
                    InjectOptions::OptionalNamed(n) => {
                        let inner = option_inner_type(field_ty).unwrap_or_else(|| {
                            panic!(
                                "#[inject(optional, name = \"...\")] requires the field \
                                 type to be Option<...>, but `{}` is not Option",
                                quote!(#field_ty)
                            )
                        });
                        quote! { #field_name: container.try_resolve_named::<#inner>(#n), }
                    }
                }
            })
            .collect(),
        _ => vec![],
    };

    // Strip #[inject(…)] attributes from fields in the emitted struct.
    let mut clean = input_struct.clone();
    if let Fields::Named(ref mut fields) = clean.fields {
        for field in fields.named.iter_mut() {
            field
                .attrs
                .retain(|a| a.path().get_ident().map(|i| i != "inject").unwrap_or(true));
        }
    }

    let expanded = quote! {
        #clean

        impl ::ravix::Injectable for #struct_name {
            fn construct(container: &::ravix::Container) -> ::std::sync::Arc<Self> {
                ::std::sync::Arc::new(Self {
                    #(#field_assignments)*
                })
            }
        }

        #(#binding_check_submissions)*
    };

    expanded.into()
}
