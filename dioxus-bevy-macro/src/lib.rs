use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, ItemFn, ReturnType, FnArg, Pat, PatType};

/// Transform a Bevy setup function into a Dioxus component
///
/// # Example
///
/// ```rust
/// #[bevy_component]
/// fn triangle_scene(app: &mut App) {
///     app.add_systems(Startup, setup_triangle);
/// }
/// ```
///
/// With signal props:
///
/// ```rust
/// #[bevy_component]
/// fn gltf_scene(app: &mut App, light_enabled: ReadOnlySignal<bool>, speed: ReadOnlySignal<f32>) {
///     app.add_systems(Startup, setup_scene);
/// }
/// ```
///
/// Generates a Dioxus component that can be used like:
///
/// ```rust
/// rsx! {
///     TriangleScene {}
///     GltfScene { light_enabled: my_signal, speed: speed_signal }
/// }
/// ```
#[proc_macro_attribute]
pub fn bevy_component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let fn_name = &input.sig.ident;
    let fn_body = &input.block;
    let fn_vis = &input.vis;

    // Parse function parameters
    let mut app_param = None;
    let mut signal_params = Vec::new();

    for param in &input.sig.inputs {
        if let FnArg::Typed(PatType { pat, ty, .. }) = param {
            if let Pat::Ident(pat_ident) = &**pat {
                let param_name = &pat_ident.ident;
                let param_type = &**ty;

                // First parameter should be `app: &mut App`
                if app_param.is_none() {
                    app_param = Some(param_name.clone());
                } else {
                    // Other parameters are signals
                    signal_params.push((param_name.clone(), param_type.clone()));
                }
            }
        }
    }

    // Check if function returns something (for message handler)
    let has_message_handler = !matches!(input.sig.output, ReturnType::Default);

    // Convert snake_case to PascalCase
    let component_name = to_pascal_case(&fn_name.to_string());
    let component_ident = format_ident!("{}", component_name);

    // Generate props struct if we have signal parameters
    let (props_def, component_params, prop_fields, use_effect_hooks) = if signal_params.is_empty() {
        (quote! {}, quote! {}, quote! {}, quote! {})
    } else {
        let prop_names: Vec<_> = signal_params.iter().map(|(name, _)| name).collect();
        let prop_types: Vec<_> = signal_params.iter().map(|(_, ty)| ty).collect();

        let props_struct_name = format_ident!("{}Props", component_name);

        let props_def = quote! {
            #[derive(Props, Clone, PartialEq)]
            struct #props_struct_name {
                #(#prop_names: #prop_types,)*
            }
        };

        let component_params = quote! { props: #props_struct_name };

        let prop_fields = quote! {
            #(let #prop_names = props.#prop_names;)*
        };

        // Generate use_effect hooks to send signal updates to Bevy
        // Each signal parameter gets its own use_effect that watches for changes
        let use_effect_hooks = quote! {
            #(
                {
                    let send_to_bevy = send_to_bevy.clone();
                    let signal = #prop_names;
                    use_effect(move || {
                        let value = signal();
                        send_to_bevy.send_signal_update(stringify!(#prop_names), value);
                    });
                }
            )*
        };

        (props_def, component_params, prop_fields, use_effect_hooks)
    };

    let component_signature = if signal_params.is_empty() {
        quote! {}
    } else {
        component_params
    };

    let expanded = if has_message_handler {
        // Function returns a message handler (not implemented yet)
        quote! {
            #props_def

            #[allow(non_snake_case)]
            #fn_vis fn #component_ident(#component_signature) -> dioxus::prelude::Element {
                use dioxus::prelude::*;
                use dioxus_core::current_scope_id;
                use dioxus_bevy::{BevyComponent, BevyAppRenderer};
                use std::sync::Arc;

                #prop_fields

                let instance_id = current_scope_id();
                let send_to_bevy = dioxus_bevy::use_bevy_message(instance_id);
                #use_effect_hooks

                rsx! {
                    BevyComponent {
                        instance_id,
                        factory: Arc::new(|device| {
                            Box::new(BevyAppRenderer::new(device, |app| {
                                let handler = (|| #fn_body)();
                                // TODO: Store handler for later use
                                #fn_body
                            }))
                        }),
                    }
                }
            }
        }
    } else {
        // Function only sets up Bevy app
        quote! {
            #props_def

            #[allow(non_snake_case)]
            #fn_vis fn #component_ident(#component_signature) -> dioxus::prelude::Element {
                use dioxus::prelude::*;
                use dioxus_core::current_scope_id;
                use dioxus_bevy::{BevyComponent, BevyAppRenderer};
                use std::sync::Arc;

                #prop_fields

                let instance_id = current_scope_id();
                let send_to_bevy = dioxus_bevy::use_bevy_message(instance_id);
                #use_effect_hooks

                rsx! {
                    BevyComponent {
                        instance_id,
                        factory: Arc::new(|device| {
                            Box::new(BevyAppRenderer::new(device, |app| #fn_body))
                        }),
                    }
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Convert snake_case to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}
