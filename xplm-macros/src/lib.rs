//! `#[plugin(...)]` and `#[derive(DataRefContainer)]` — sugar over the
//! hand-written mechanisms in `xplm::plugin`/`xplm::dataref` (Phases 3-4).
//! Neither macro invents new runtime behavior; they only generate the same
//! code a caller could write by hand.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Data, DeriveInput, Expr, Fields, Ident, ItemStruct, Lit, LitStr, Meta, Token};

struct NameValueLit {
    ident: Ident,
    value: LitStr,
}

impl Parse for NameValueLit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let value: LitStr = input.parse()?;
        Ok(Self { ident, value })
    }
}

struct PluginArgs {
    name: LitStr,
    signature: LitStr,
    description: LitStr,
}

impl Parse for PluginArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let pairs = Punctuated::<NameValueLit, Token![,]>::parse_terminated(input)?;
        let (mut name, mut signature, mut description) = (None, None, None);
        for pair in pairs {
            match pair.ident.to_string().as_str() {
                "name" => name = Some(pair.value),
                "signature" => signature = Some(pair.value),
                "description" => description = Some(pair.value),
                other => {
                    return Err(syn::Error::new(
                        pair.ident.span(),
                        format!(
                            "unknown #[plugin] argument `{other}`; expected `name`, `signature`, or `description`"
                        ),
                    ));
                }
            }
        }
        Ok(Self {
            name: name.ok_or_else(|| input.error("#[plugin] requires `name = \"...\"`"))?,
            signature: signature
                .ok_or_else(|| input.error("#[plugin] requires `signature = \"...\"`"))?,
            description: description
                .ok_or_else(|| input.error("#[plugin] requires `description = \"...\"`"))?,
        })
    }
}

/// Attribute macro for a plugin's top-level state struct:
///
/// ```ignore
/// #[xplm::plugin(
///     name = "My Awesome Rust Plugin",
///     signature = "com.example.my-plugin",
///     description = "Doing physics things in Rust"
/// )]
/// struct MyPlugin { /* ... */ }
///
/// impl xplm::plugin::XPlanePlugin for MyPlugin {
///     fn start() -> Self { /* ... */ }
///     // enable/disable/stop/receive_message as needed; NAME/SIGNATURE/
///     // DESCRIPTION are supplied by this attribute, not overridden here.
/// }
/// ```
///
/// Expands to the struct definition unchanged, plus
/// `xplm::register_plugin!(MyPlugin, name = ..., signature = ..., description
/// = ...)` — the metadata-argument form, so the `impl XPlanePlugin` block
/// doesn't need to (and shouldn't) override the trait's `NAME`/`SIGNATURE`/
/// `DESCRIPTION` defaults itself.
#[proc_macro_attribute]
pub fn plugin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as PluginArgs);
    let item_struct = syn::parse_macro_input!(item as ItemStruct);
    let ident = &item_struct.ident;

    let name = args.name;
    let signature = args.signature;
    let description = args.description;

    quote! {
        #item_struct

        ::xplm::register_plugin!(
            #ident,
            name = #name,
            signature = #signature,
            description = #description
        );
    }
    .into()
}

/// Derives a `find() -> Option<Self>` for a struct whose fields are each
/// `ReadOnly<T>`/`ReadWrite<T>` (or `ArrayDataRef` variants), tagged with
/// `#[dataref = "sim/path/to/dataref"]`:
///
/// ```ignore
/// #[derive(xplm::DataRefContainer)]
/// struct AircraftTelemetry {
///     #[dataref = "sim/flightmodel/position/latitude"]
///     latitude: xplm::dataref::ReadOnly<f64>,
///     #[dataref = "sim/cockpit2/engine/actuators/throttle_ratio_all"]
///     throttle: xplm::dataref::ReadWrite<f32>,
/// }
/// ```
///
/// `find()` looks up every field via that type's own `find` (already
/// generic over the `ReadOnly`/`ReadWrite` access marker from Phase 3 —
/// this macro doesn't need to know which), returning `None` as soon as any
/// one of them isn't currently registered.
#[proc_macro_derive(DataRefContainer, attributes(dataref))]
pub fn derive_dataref_container(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let ident = &input.ident;

    let Data::Struct(data) = &input.data else {
        return syn::Error::new_spanned(
            &input,
            "#[derive(DataRefContainer)] only supports structs",
        )
        .to_compile_error()
        .into();
    };
    let Fields::Named(fields) = &data.fields else {
        return syn::Error::new_spanned(
            &data.fields,
            "#[derive(DataRefContainer)] requires named fields",
        )
        .to_compile_error()
        .into();
    };

    let mut inits = Vec::new();
    for field in &fields.named {
        let field_ident = field.ident.as_ref().expect("named field");
        let field_ty = &field.ty;

        let dataref_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("dataref"))
            .collect();

        let path_lit = match dataref_attrs.as_slice() {
            [attr] => match path_lit_from_attr(attr) {
                Ok(lit) => lit,
                Err(err) => return err.to_compile_error().into(),
            },
            [] => {
                return syn::Error::new_spanned(
                    field_ident,
                    "fields of a #[derive(DataRefContainer)] struct need a \
                     #[dataref = \"sim/...\"] attribute",
                )
                .to_compile_error()
                .into();
            }
            [_, extra, ..] => {
                return syn::Error::new_spanned(extra, "only one #[dataref = \"...\"] allowed")
                    .to_compile_error()
                    .into();
            }
        };

        inits.push(quote! {
            #field_ident: <#field_ty>::find(#path_lit)?
        });
    }

    quote! {
        impl #ident {
            /// Looks up every `#[dataref = "..."]` field, returning `None`
            /// if any of them isn't currently registered.
            pub fn find() -> ::std::option::Option<Self> {
                ::std::option::Option::Some(Self {
                    #(#inits),*
                })
            }
        }
    }
    .into()
}

fn path_lit_from_attr(attr: &syn::Attribute) -> syn::Result<LitStr> {
    let Meta::NameValue(nv) = &attr.meta else {
        return Err(syn::Error::new_spanned(
            attr,
            "expected `#[dataref = \"sim/...\"]`",
        ));
    };
    let Expr::Lit(expr_lit) = &nv.value else {
        return Err(syn::Error::new_spanned(&nv.value, "expected a string literal"));
    };
    let Lit::Str(s) = &expr_lit.lit else {
        return Err(syn::Error::new_spanned(&expr_lit.lit, "expected a string literal"));
    };
    Ok(s.clone())
}
