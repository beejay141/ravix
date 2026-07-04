use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Attribute, FnArg, ImplItem, ItemImpl, LitStr, Path};

const HTTP_METHODS: &[&str] = &["get", "post", "put", "delete", "patch"];

struct HandlerInfo {
    verb: String,                // "GET"
    path: String,                // "/:id"
    fn_name: syn::Ident,         // get_user
    middlewares: Vec<Path>,      // [auth_guard]
    non_self_inputs: Vec<FnArg>, // non-receiver parameters
    has_self: bool,              // true when first param is &self
}

pub fn expand(args: TokenStream, input: TokenStream) -> TokenStream {
    let base_path = parse_macro_input!(args as LitStr).value();
    let mut impl_block = parse_macro_input!(input as ItemImpl);

    let self_ty = impl_block.self_ty.clone();

    let mut handler_infos: Vec<HandlerInfo> = Vec::new();
    let mut clean_items: Vec<ImplItem> = Vec::new();

    for item in impl_block.items.drain(..) {
        match item {
            ImplItem::Fn(mut method) => {
                let mut http_verb: Option<String> = None;
                let mut http_path: Option<String> = None;
                let mut middlewares: Vec<Path> = Vec::new();
                let mut remaining: Vec<Attribute> = Vec::new();

                for attr in method.attrs.drain(..) {
                    let name = attr
                        .path()
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();

                    if HTTP_METHODS.contains(&name.as_str()) {
                        http_verb = Some(name.to_uppercase());
                        let lit: LitStr = attr.parse_args().unwrap_or_else(|_| {
                            panic!(
                                "#[{}] requires a path string, e.g. #[{}(\"/\")]",
                                name, name
                            )
                        });
                        http_path = Some(lit.value());
                    } else if name == "middleware" {
                        let guard: Path = attr.parse_args().unwrap_or_else(|_| {
                            panic!(
                                "#[middleware] requires a function path, \
                                 e.g. #[middleware(auth_guard)]"
                            )
                        });
                        middlewares.push(guard);
                    } else {
                        remaining.push(attr);
                    }
                }

                method.attrs = remaining;

                if let (Some(verb), Some(path)) = (http_verb, http_path) {
                    let has_self = method
                        .sig
                        .inputs
                        .iter()
                        .any(|arg| matches!(arg, FnArg::Receiver(_)));
                    let non_self_inputs: Vec<FnArg> = method
                        .sig
                        .inputs
                        .iter()
                        .filter(|arg| !matches!(arg, FnArg::Receiver(_)))
                        .cloned()
                        .collect();
                    handler_infos.push(HandlerInfo {
                        verb,
                        path,
                        fn_name: method.sig.ident.clone(),
                        middlewares,
                        non_self_inputs,
                        has_self,
                    });
                }

                clean_items.push(ImplItem::Fn(method));
            }
            other => clean_items.push(other),
        }
    }

    impl_block.items = clean_items;

    let registrations: Vec<TokenStream2> = handler_infos
        .iter()
        .enumerate()
        .map(|(idx, info)| {
            let fn_name = &info.fn_name;
            let verb_str = &info.verb;
            let base_str = &base_path;
            let route_str = &info.path;

            // Unique factory fn name per route (type + method name + index)
            let self_ty_str = quote!(#self_ty)
                .to_string()
                .replace("::", "_")
                .replace(' ', "");
            let factory_ident = format_ident!("__ravix_route_{}_{}_{idx}", self_ty_str, fn_name);

            // Build the base MethodRouter — two modes:
            // • has_self=true  → resolve Arc<Self> from container once at startup,
            //                    capture in a closure (zero container cost per request)
            // • has_self=false → free-function handler: method IS the axum handler directly
            let (preamble, routing_call) = if info.has_self {
                let arg_names: Vec<_> = (0..info.non_self_inputs.len())
                    .map(|i| format_ident!("__arg{}", i))
                    .collect();
                let arg_types: Vec<_> = info
                    .non_self_inputs
                    .iter()
                    .filter_map(|arg| {
                        if let FnArg::Typed(pt) = arg {
                            Some(pt.ty.as_ref())
                        } else {
                            None
                        }
                    })
                    .collect();

                // Resolve the controller singleton once when the router is built.
                let pre = quote! {
                    let __ctrl = __container.resolve::<::std::sync::Arc<#self_ty>>();
                };

                // Each request clones the Arc (atomic refcount bump only).
                let routing = match info.verb.as_str() {
                    "GET" => quote! {
                        ::axum::routing::get(move |#(#arg_names: #arg_types),*| {
                            let __ctrl = __ctrl.clone();
                            async move { __ctrl.#fn_name(#(#arg_names),*).await }
                        })
                    },
                    "POST" => quote! {
                        ::axum::routing::post(move |#(#arg_names: #arg_types),*| {
                            let __ctrl = __ctrl.clone();
                            async move { __ctrl.#fn_name(#(#arg_names),*).await }
                        })
                    },
                    "PUT" => quote! {
                        ::axum::routing::put(move |#(#arg_names: #arg_types),*| {
                            let __ctrl = __ctrl.clone();
                            async move { __ctrl.#fn_name(#(#arg_names),*).await }
                        })
                    },
                    "DELETE" => quote! {
                        ::axum::routing::delete(move |#(#arg_names: #arg_types),*| {
                            let __ctrl = __ctrl.clone();
                            async move { __ctrl.#fn_name(#(#arg_names),*).await }
                        })
                    },
                    "PATCH" => quote! {
                        ::axum::routing::patch(move |#(#arg_names: #arg_types),*| {
                            let __ctrl = __ctrl.clone();
                            async move { __ctrl.#fn_name(#(#arg_names),*).await }
                        })
                    },
                    _ => panic!("Unknown HTTP verb: {}", info.verb),
                };

                (pre, routing)
            } else {
                let routing = match info.verb.as_str() {
                    "GET" => quote! { ::axum::routing::get(#self_ty::#fn_name)    },
                    "POST" => quote! { ::axum::routing::post(#self_ty::#fn_name)   },
                    "PUT" => quote! { ::axum::routing::put(#self_ty::#fn_name)    },
                    "DELETE" => quote! { ::axum::routing::delete(#self_ty::#fn_name) },
                    "PATCH" => quote! { ::axum::routing::patch(#self_ty::#fn_name)  },
                    _ => panic!("Unknown HTTP verb: {}", info.verb),
                };
                (quote! {}, routing)
            };

            // Wrap with per-route middleware layers (innermost first).
            // from_fn_with_state passes the ContainerRef as axum state so
            // middleware functions can optionally accept State<ContainerRef>.
            let with_layers = info.middlewares.iter().fold(routing_call, |acc, guard| {
                quote! {
                    #acc.route_layer(::axum::middleware::from_fn_with_state(
                        ::std::sync::Arc::clone(__container),
                        #guard,
                    ))
                }
            });

            quote! {
                #[allow(non_snake_case)]
                fn #factory_ident(__container: &::ravix::ContainerRef) -> ::axum::routing::MethodRouter<::ravix::ContainerRef> {
                    #preamble
                    #with_layers
                }

                ::ravix::inventory::submit!(::ravix::RouteDescriptor {
                    method: #verb_str,
                    base_path: #base_str,
                    path: #route_str,
                    handler: #factory_ident,
                });
            }
        })
        .collect();

    let expanded = quote! {
        #impl_block
        #(#registrations)*
    };

    expanded.into()
}
