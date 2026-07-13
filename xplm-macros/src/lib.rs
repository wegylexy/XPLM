//! `#[plugin(...)]` and `#[derive(DataRefContainer)]` — sugar over the
//! hand-written mechanisms in `xplm::plugin`/`xplm::dataref` (Phases 3-4).
//! Neither macro invents new runtime behavior; they only generate the same
//! code a caller could write by hand.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Data, DeriveInput, Expr, Fields, GenericArgument, Ident, ItemStruct, Lit, LitStr, Meta,
    PathArguments, Token, Type,
};

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

/// Derives a `find()` for a struct tagged field-by-field with
/// `#[dataref = "sim/path/to/dataref"]`, in either of two field-type styles,
/// which *can* be mixed in one struct (see below). An optional struct-level
/// `#[dataref_prefix = "..."]` is prepended to any field that omits its own
/// `#[dataref = "..."]`, using the field's name as the suffix — same
/// attribute, same behavior, as [`PublishedDataRefContainer`]'s.
///
/// - **Wrapper-typed** — a field already typed as `xplm::dataref::ReadOnly<T>`/
///   `ReadWrite<T>` (or an `ArrayDataRef`/`DataBytes` variant) is looked up
///   via that type's own `find` and stored as-is; `find() -> Option<Self>`:
///
///   ```ignore
///   #[derive(xplm::DataRefContainer)]
///   struct AircraftTelemetry {
///       #[dataref = "sim/flightmodel/position/latitude"]
///       latitude: xplm::dataref::ReadOnly<f64>,
///       #[dataref = "sim/cockpit2/engine/actuators/throttle_ratio_all"]
///       throttle: xplm::dataref::ReadWrite<f32>,
///   }
///
///   let telemetry = AircraftTelemetry::find().expect("dataref(s) not found");
///   let lat = telemetry.latitude.get();
///   telemetry.throttle.set(0.75);
///   ```
///
/// - **Plain-typed** — a field typed as a plain `i32`/`f32`/`f64`/`Vec<u8>`
///   (optionally `#[writable]`) doesn't need you to spell out `ReadOnly`/
///   `ReadWrite`/`DataBytes` yourself: the derive picks the right wrapper
///   internally and exposes a getter (and, if `#[writable]`, a `set_*`
///   setter) instead. Since there's no per-field wrapper type to store
///   *in* `Self` this way, `find()` returns a companion `FooHandle` (same
///   naming/shape as [`PublishedDataRefContainer`]'s, minus the
///   `Rc<RefCell<_>>` — there's no local state to buffer here, every getter/
///   setter is a live `XPLMGetData*`/`XPLMSetData*` call) instead of `Self`:
///
///   ```ignore
///   #[derive(xplm::DataRefContainer)]
///   struct NavRadio {
///       #[dataref = "MyAvionics/Nav1/frequency_khz"]
///       frequency_khz: i32,
///       #[dataref = "MyAvionics/Nav1/course_deg"]
///       #[writable]
///       course_deg: f32,
///   }
///
///   let radio = NavRadio::find().expect("dataref(s) not found");
///   let freq: i32 = radio.frequency_khz(); // getter, even though not #[writable]
///   radio.set_course_deg(90.0); // setter, only exists because #[writable]
///
///   let snapshot: NavRadio = radio.get(); // every field read in one call
///   radio.set(&NavRadio { frequency_khz: 0, course_deg: 90.0 }); // every #[writable]
///                                                                // field written in one
///                                                                // call; non-writable
///                                                                // fields in the value
///                                                                // passed are ignored
///   ```
///
///   `Handle::get`/`Handle::set` are the bulk counterparts of the per-field
///   getters/setters above — `get()` builds a `Self` snapshot by calling
///   every field's getter once; `set()` takes a `&Self` and calls only the
///   `#[writable]` fields' setters, ignoring the rest (there's no setter to
///   call them through). `Self` (`NavRadio` above) still isn't constructed
///   anywhere *else* by generated code — it exists as the schema `find()`
///   reads to build `NavRadioHandle`, and as `get()`'s return type/`set()`'s
///   parameter type — so a struct that never calls `get()` may still see an
///   unused-field warning on it; feel free to `#[allow(dead_code)]`.
///
/// - **Mixing both** — a struct can have some plain-typed fields and some
///   wrapper-typed ones (restricted to `ReadOnly<T>`/`ReadWrite<T>`/
///   `ReadOnlyBytes`/`ReadWriteBytes` specifically — an array or a raw
///   `DataRef<T, A>` mixed in this way is a compile error, since there's no
///   single plain value to represent it with below). Wrapper-typed fields
///   are stored in the `Handle` exactly as declared (same direct
///   `.get()`/`.set()` access as the pure wrapper-typed style); plain-typed
///   fields still get a generated getter/setter. Since `Self` can't serve as
///   the bulk snapshot type anymore (its wrapper-typed fields aren't plain
///   values), `get()`/`set()` use a separate generated `FooSnapshot`
///   instead, with a plain value for every field:
///
///   ```ignore
///   #[derive(xplm::DataRefContainer)]
///   #[dataref_prefix = "MyAvionics/Nav1/"]
///   struct NavRadio {
///       frequency_khz: i32, // plain
///       #[dataref = "MyAvionics/Shared/active_nav_ident"]
///       ident: xplm::dataref::ReadWriteBytes, // wrapper-typed
///   }
///
///   let radio = NavRadio::find().expect("dataref(s) not found");
///   let freq: i32 = radio.frequency_khz(); // generated getter (plain field)
///   radio.ident.set(0, b"KABC"); // direct field access (wrapper-typed field)
///
///   let snapshot: NavRadioSnapshot = radio.get(); // both kinds of field in one snapshot
///   radio.set(&NavRadioSnapshot { frequency_khz: 0, ident: b"KDEF".to_vec() });
///   ```
///
/// Either way, `find()` returns `None` as soon as any one field's dataref
/// isn't currently registered.
#[proc_macro_derive(
    DataRefContainer,
    attributes(dataref, dataref_prefix, writable, packed)
)]
pub fn derive_dataref_container(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let ident = &input.ident;
    let vis = &input.vis;

    let prefix = match dataref_prefix_from_attrs(&input.attrs) {
        Ok(prefix) => prefix,
        Err(err) => return err.to_compile_error().into(),
    };

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

    let mut field_paths = Vec::new();
    for field in &fields.named {
        let field_ident = field.ident.as_ref().expect("named field");

        let dataref_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("dataref"))
            .collect();

        let path_lit = match dataref_attrs.as_slice() {
            // An explicit #[dataref = "..."] always wins outright — it's
            // the full path, not a suffix, even under #[dataref_prefix].
            [attr] => match path_lit_from_attr(attr) {
                Ok(lit) => lit,
                Err(err) => return err.to_compile_error().into(),
            },
            // No per-field override: fall back to `prefix + field name`, so
            // #[dataref_prefix = "..."] only saves you from repeating a
            // shared prefix on every field, it never *requires* one.
            [] if !prefix.is_empty() => {
                LitStr::new(&format!("{prefix}{field_ident}"), field_ident.span())
            }
            [] => {
                return syn::Error::new_spanned(
                    field_ident,
                    "fields of a #[derive(DataRefContainer)] struct need a \
                     #[dataref = \"sim/...\"] attribute (or the struct needs a \
                     #[dataref_prefix = \"...\"] for this field to fall back to)",
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
        field_paths.push(path_lit);
    }

    let plain_count = fields.named.iter().filter(|f| field_is_plain(f)).count();

    if plain_count == 0 {
        // Pure wrapper-typed style: unchanged from the original behavior,
        // and fully permissive on field type (arrays, raw DataRef<T, A>,
        // anything with its own `find`) — no Handle/Snapshot involved.
        let inits = fields
            .named
            .iter()
            .zip(&field_paths)
            .map(|(field, path_lit)| {
                let field_ident = field.ident.as_ref().expect("named field");
                let field_ty = &field.ty;
                quote! {
                    #field_ident: <#field_ty>::find(#path_lit)?
                }
            });

        return quote! {
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
        .into();
    }

    // At least one plain-typed field: generate a companion FooHandle, same
    // shape as PublishedDataRefContainer's — minus the Rc<RefCell<_>>, since
    // every getter/setter here is a live XPLMGetData*/XPLMSetData* call, not
    // a read/write against locally-buffered state.
    //
    // A field here is one of three kinds:
    // - Plain scalar/Vec<u8> (i32/f32/f64/Vec<u8>, optionally #[writable]):
    //   the derive picks the wrapper, stores it as a private Handle field,
    //   and generates a getter/(setter) method.
    // - Plain packed (#[packed], any type T you assert impls PackedDataRef):
    //   same as above, but through ReadOnlyStruct<T>/ReadWriteStruct<T> —
    //   the getter returns Option<T> (a byte-length mismatch is possible;
    //   see PackedDataRef's doc comment), not T directly.
    // - Recognized wrapper (ReadOnly<T>/ReadWrite<T>/ReadOnlyBytes/
    //   ReadWriteBytes/ReadOnlyStruct<T>/ReadWriteStruct<T>, written out by
    //   hand): stored in the Handle exactly as declared, as a public field —
    //   access it the same way you would on the old wrapper-typed `Self`,
    //   via `.get()`/`.set()` directly.
    // Any other wrapper type (arrays, a raw `DataRef<T, A>`, ...) mixed in
    // with a plain field is a compile error below — those don't have a
    // single well-defined "plain value" to put in the bulk Snapshot type.
    let any_packed = fields.named.iter().any(|f| {
        f.attrs.iter().any(|attr| attr.path().is_ident("packed"))
            || matches!(classify_wrapper(&f.ty), Some(WrapperKind::Packed { .. }))
    });
    // Self can serve as its own snapshot type only if every field is a
    // plain scalar/Vec<u8> — a packed field's Handle getter returns
    // Option<T>, which Self's own field (declared as plain T) can't hold.
    let use_self_as_snapshot = !any_packed && plain_count == fields.named.len();
    let snapshot_ident = if use_self_as_snapshot {
        ident.clone()
    } else {
        format_ident!("{}Snapshot", ident)
    };

    let handle_ident = format_ident!("{}Handle", ident);
    let mut handle_fields = Vec::new();
    let mut find_inits = Vec::new();
    let mut accessor_methods = Vec::new();
    let mut snapshot_fields = Vec::new();
    let mut get_all_inits = Vec::new();
    let mut set_all_calls = Vec::new();

    for (field, path_lit) in fields.named.iter().zip(&field_paths) {
        let field_ident = field.ident.as_ref().expect("named field");
        let field_ty = &field.ty;
        let set_ident = format_ident!("set_{}", field_ident);

        let packed = field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("packed"));

        if packed || is_plain_type(field_ty) {
            let writable = field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("writable"));

            if packed {
                // #[packed]: T is asserted by the caller to impl
                // PackedDataRef; the getter returns Option<T>, not T, since
                // a byte-length mismatch is possible (see PackedDataRef's
                // doc comment) — so the Snapshot field is Option<T> too,
                // and set() only calls through when a value is present.
                let wrapper_ty = if writable {
                    quote!(::xplm::dataref::ReadWriteStruct<#field_ty>)
                } else {
                    quote!(::xplm::dataref::ReadOnlyStruct<#field_ty>)
                };
                handle_fields.push(quote!(#field_ident: #wrapper_ty));
                find_inits.push(quote! {
                    #field_ident: <#wrapper_ty>::find(#path_lit)?
                });
                accessor_methods.push(quote! {
                    #vis fn #field_ident(&self) -> ::std::option::Option<#field_ty> {
                        self.#field_ident.get()
                    }
                });
                if writable {
                    accessor_methods.push(quote! {
                        #vis fn #set_ident(&self, value: #field_ty) {
                            self.#field_ident.set(value);
                        }
                    });
                }

                snapshot_fields.push(quote!(#vis #field_ident: ::std::option::Option<#field_ty>));
                get_all_inits.push(quote! {
                    #field_ident: self.#field_ident()
                });
                if writable {
                    set_all_calls.push(quote! {
                        if let ::std::option::Option::Some(value) = values.#field_ident {
                            self.#set_ident(value);
                        }
                    });
                }
                continue;
            }

            snapshot_fields.push(quote!(#vis #field_ident: #field_ty));
            get_all_inits.push(quote! {
                #field_ident: self.#field_ident()
            });
            if writable {
                set_all_calls.push(quote! {
                    self.#set_ident(values.#field_ident.clone());
                });
            }

            if is_vec_u8(field_ty) {
                let wrapper_ty = if writable {
                    quote!(::xplm::dataref::ReadWriteBytes)
                } else {
                    quote!(::xplm::dataref::ReadOnlyBytes)
                };
                handle_fields.push(quote!(#field_ident: #wrapper_ty));
                find_inits.push(quote! {
                    #field_ident: <#wrapper_ty>::find(#path_lit)?
                });
                accessor_methods.push(quote! {
                    #vis fn #field_ident(&self) -> #field_ty {
                        self.#field_ident.get_all()
                    }
                });
                if writable {
                    accessor_methods.push(quote! {
                        #vis fn #set_ident(&self, value: #field_ty) {
                            self.#field_ident.set(0, &value);
                        }
                    });
                }
            } else if let Some(elem_ty) = vec_array_elem_type(field_ty) {
                let wrapper_ty = if writable {
                    quote!(::xplm::dataref::ReadWriteArray<#elem_ty>)
                } else {
                    quote!(::xplm::dataref::ReadOnlyArray<#elem_ty>)
                };
                handle_fields.push(quote!(#field_ident: #wrapper_ty));
                find_inits.push(quote! {
                    #field_ident: <#wrapper_ty>::find(#path_lit)?
                });
                accessor_methods.push(quote! {
                    #vis fn #field_ident(&self) -> #field_ty {
                        self.#field_ident.get_all()
                    }
                });
                if writable {
                    accessor_methods.push(quote! {
                        #vis fn #set_ident(&self, value: #field_ty) {
                            self.#field_ident.set(0, &value);
                        }
                    });
                }
            } else {
                let wrapper_ty = if writable {
                    quote!(::xplm::dataref::ReadWrite<#field_ty>)
                } else {
                    quote!(::xplm::dataref::ReadOnly<#field_ty>)
                };
                handle_fields.push(quote!(#field_ident: #wrapper_ty));
                find_inits.push(quote! {
                    #field_ident: <#wrapper_ty>::find(#path_lit)?
                });
                accessor_methods.push(quote! {
                    #vis fn #field_ident(&self) -> #field_ty {
                        self.#field_ident.get()
                    }
                });
                if writable {
                    accessor_methods.push(quote! {
                        #vis fn #set_ident(&self, value: #field_ty) {
                            self.#field_ident.set(value);
                        }
                    });
                }
            }
            continue;
        }

        // Not plain-typed. Since plain_count > 0, at least one other field
        // IS plain-typed, so this is the genuine-mix case — restrict to the
        // recognized wrapper aliases so there's a well-defined plain value
        // to put in the Snapshot.
        let Some(wrapper) = classify_wrapper(field_ty) else {
            return syn::Error::new_spanned(
                field_ty,
                "when mixing plain-typed fields (`i32`/`f32`/`f64`/`Vec<u8>`/`Vec<i32>`/\
                 `Vec<f32>`, or #[packed]) with wrapper-typed ones in the same \
                 #[derive(DataRefContainer)] struct, the wrapper-typed fields must be \
                 `ReadOnly<T>`/`ReadWrite<T>`/`ReadOnlyBytes`/`ReadWriteBytes`/\
                 `ReadOnlyStruct<T>`/`ReadWriteStruct<T>`/`ReadOnlyArray<T>`/`ReadWriteArray<T>` \
                 — other wrapper types (a raw `DataRef<T, A>`/`ArrayDataRef<T, A>`, ...) have no \
                 single plain value to put in the generated Snapshot type. Use a struct with \
                 only recognized wrapper types (any field type allowed) or only plain/#[packed] \
                 types instead.",
            )
            .to_compile_error()
            .into();
        };

        // Stored in the Handle exactly as declared — same direct
        // `.get()`/`.set()` ergonomics as the pure wrapper-typed style.
        handle_fields.push(quote!(#vis #field_ident: #field_ty));
        find_inits.push(quote! {
            #field_ident: <#field_ty>::find(#path_lit)?
        });

        match wrapper {
            WrapperKind::Scalar { inner, writable } => {
                snapshot_fields.push(quote!(#vis #field_ident: #inner));
                get_all_inits.push(quote! {
                    #field_ident: self.#field_ident.get()
                });
                if writable {
                    set_all_calls.push(quote! {
                        self.#field_ident.set(values.#field_ident.clone());
                    });
                }
            }
            WrapperKind::Bytes { writable } => {
                snapshot_fields.push(quote!(#vis #field_ident: ::std::vec::Vec<u8>));
                get_all_inits.push(quote! {
                    #field_ident: self.#field_ident.get_all()
                });
                if writable {
                    set_all_calls.push(quote! {
                        self.#field_ident.set(0, &values.#field_ident);
                    });
                }
            }
            WrapperKind::Packed { inner, writable } => {
                snapshot_fields.push(quote!(#vis #field_ident: ::std::option::Option<#inner>));
                get_all_inits.push(quote! {
                    #field_ident: self.#field_ident.get()
                });
                if writable {
                    set_all_calls.push(quote! {
                        if let ::std::option::Option::Some(value) = values.#field_ident {
                            self.#field_ident.set(value);
                        }
                    });
                }
            }
            WrapperKind::Array { inner, writable } => {
                snapshot_fields.push(quote!(#vis #field_ident: ::std::vec::Vec<#inner>));
                get_all_inits.push(quote! {
                    #field_ident: self.#field_ident.get_all()
                });
                if writable {
                    set_all_calls.push(quote! {
                        self.#field_ident.set(0, &values.#field_ident);
                    });
                }
            }
        }
    }

    // Only emit a Snapshot struct definition for the genuine-mix case — for
    // the all-plain case, Self already *is* the struct the user wrote, and
    // re-declaring it here would conflict.
    let snapshot_decl = if use_self_as_snapshot {
        quote!()
    } else {
        quote! {
            /// Bulk-value snapshot `Handle::get`/`Handle::set` use — see
            /// `#[derive(DataRefContainer)]`'s doc comment.
            #vis struct #snapshot_ident {
                #(#snapshot_fields),*
            }
        }
    };

    quote! {
        #snapshot_decl

        /// Live getter/setter accessor `find()` produces for this schema —
        /// see `#[derive(DataRefContainer)]`'s doc comment.
        #vis struct #handle_ident {
            #(#handle_fields),*
        }

        impl #handle_ident {
            #(#accessor_methods)*

            /// Reads every field via its getter, returning a one-shot
            /// snapshot of this dataref group's current values.
            #vis fn get(&self) -> #snapshot_ident {
                #snapshot_ident {
                    #(#get_all_inits),*
                }
            }

            /// Writes every `#[writable]`/`ReadWrite`-typed field of
            /// `values` via its setter. Fields that aren't writable are
            /// ignored — there's no setter to call them through, so their
            /// values in `values` don't matter.
            #vis fn set(&self, values: &#snapshot_ident) {
                #(#set_all_calls)*
            }
        }

        impl #ident {
            /// Looks up every `#[dataref = "..."]` field, returning `None`
            /// if any of them isn't currently registered.
            #vis fn find() -> ::std::option::Option<#handle_ident> {
                ::std::option::Option::Some(#handle_ident {
                    #(#find_inits),*
                })
            }
        }
    }
    .into()
}

/// Derives a `publish(self) -> Option<FooHandle>` for a struct whose fields
/// hold the *plain values* backing published datarefs —
/// `i32`/`f32`/`f64` (via [`xplm::dataref::PublishedDataRef`]) or `Vec<u8>`
/// (via [`xplm::dataref::PublishedData`], X-Plane's `xplmType_Data`
/// byte-string type) — tagged with `#[dataref = "path/to/dataref"]` and, for
/// a writable dataref, also `#[writable]`. An optional struct-level
/// `#[dataref_prefix = "..."]` is prepended to every field that omits its own
/// `#[dataref = "..."]`, using the field's own name as the suffix; a field
/// that does specify `#[dataref = "..."]` uses that path exactly as given,
/// skipping the prefix entirely (it's a full override, not a suffix):
///
/// ```ignore
/// #[derive(xplm::PublishedDataRefContainer)]
/// #[dataref_prefix = "MyAvionics/Nav1/"]
/// struct NavRadio {
///     frequency_khz: i32,     // -> "MyAvionics/Nav1/frequency_khz"
///     #[writable]
///     course_deg: f32,        // -> "MyAvionics/Nav1/course_deg"
///     #[dataref = "MyAvionics/Shared/active_nav_ident"] // explicit: skips the prefix
///     #[writable]
///     ident: Vec<u8>,
/// }
/// ```
///
/// expands to a `NavRadioHandle` — a cheap-to-`Clone` accessor with one
/// getter (and, for `#[writable]` fields, one `set_*` setter) per field:
///
/// ```ignore
/// impl NavRadio {
///     fn publish(self) -> Option<NavRadioHandle> { /* ... */ }
/// }
///
/// impl NavRadioHandle {
///     fn frequency_khz(&self) -> i32; // no setter: frequency_khz isn't #[writable]
///     fn course_deg(&self) -> f32;
///     fn set_course_deg(&self, value: f32);
///     fn ident(&self) -> Vec<u8>;
///     fn set_ident(&self, value: Vec<u8>);
/// }
/// ```
///
/// `publish` takes `self` by value and does the `Rc::new(RefCell::new(self))`
/// wrapping itself — every dataref accessor closure, and every
/// `NavRadioHandle` method, reads/writes the matching field through that
/// shared state (same `Rc<RefCell<_>>`-around-the-whole-state shape as
/// [`xpmp2::Aircraft`]'s recommended pattern for reaching a placed `Plane`'s
/// state from outside `update_position`; this derive just does the
/// wrapping, and the `borrow()`/`borrow_mut()` calls, for you instead of
/// making you write them at every call site). Getters exist for every
/// field, `#[writable]` or not — that attribute only gates whether *other*
/// plugins can set the dataref via `XPLMSetData*`, not whether your own code
/// can read/write the field through the `Handle`. `NavRadioHandle` is cheap
/// to `Clone` (every clone shares the same underlying state and
/// registrations), and every published dataref stays registered — and
/// unregisters automatically — for exactly as long as *any* clone of it is
/// alive; there's no second value to separately hold onto or drop. `publish`
/// returns `None` as soon as any one field's `XPLMRegisterDataAccessor` call
/// fails, dropping (and so unregistering) whichever fields it already
/// registered — `self` is then lost along with it, same as any other
/// constructor that can fail after taking ownership.
#[proc_macro_derive(
    PublishedDataRefContainer,
    attributes(dataref, dataref_prefix, writable, packed)
)]
pub fn derive_published_dataref_container(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let ident = &input.ident;
    let vis = &input.vis;
    let registrations_ident = format_ident!("{}Registrations", ident);
    let handle_ident = format_ident!("{}Handle", ident);

    let prefix = match dataref_prefix_from_attrs(&input.attrs) {
        Ok(prefix) => prefix,
        Err(err) => return err.to_compile_error().into(),
    };

    let Data::Struct(data) = &input.data else {
        return syn::Error::new_spanned(
            &input,
            "#[derive(PublishedDataRefContainer)] only supports structs",
        )
        .to_compile_error()
        .into();
    };
    let Fields::Named(fields) = &data.fields else {
        return syn::Error::new_spanned(
            &data.fields,
            "#[derive(PublishedDataRefContainer)] requires named fields",
        )
        .to_compile_error()
        .into();
    };

    let mut registration_fields = Vec::new();
    let mut publish_inits = Vec::new();
    let mut accessor_methods = Vec::new();

    for field in &fields.named {
        let field_ident = field.ident.as_ref().expect("named field");
        let field_ty = &field.ty;

        let dataref_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("dataref"))
            .collect();
        let path_lit = match dataref_attrs.as_slice() {
            // An explicit #[dataref = "..."] always wins outright — it's
            // the full path, not a suffix, even under #[dataref_prefix].
            [attr] => match path_lit_from_attr(attr) {
                Ok(lit) => lit,
                Err(err) => return err.to_compile_error().into(),
            },
            // No per-field override: fall back to `prefix + field name`, so
            // #[dataref_prefix = "..."] only saves you from repeating a
            // shared prefix on every field, it never *requires* one.
            [] if !prefix.is_empty() => {
                LitStr::new(&format!("{prefix}{field_ident}"), field_ident.span())
            }
            [] => {
                return syn::Error::new_spanned(
                    field_ident,
                    "fields of a #[derive(PublishedDataRefContainer)] struct need a \
                     #[dataref = \"sim/...\"] attribute (or the struct needs a \
                     #[dataref_prefix = \"...\"] for this field to fall back to)",
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
        let writable = field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("writable"));
        let packed = field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("packed"));

        let set_ident = format_ident!("set_{}", field_ident);

        if is_vec_u8(field_ty) {
            let handle_ty = if writable {
                quote!(::xplm::dataref::PublishedDataReadWrite)
            } else {
                quote!(::xplm::dataref::PublishedDataReadOnly)
            };
            registration_fields.push(quote!(#field_ident: #handle_ty));

            accessor_methods.push(quote! {
                #vis fn #field_ident(&self) -> #field_ty {
                    ::std::clone::Clone::clone(&(*self.state.borrow()).#field_ident)
                }
            });
            if writable {
                accessor_methods.push(quote! {
                    #vis fn #set_ident(&self, value: #field_ty) {
                        (*self.state.borrow_mut()).#field_ident = value;
                    }
                });
            }

            publish_inits.push(if writable {
                quote! {
                    #field_ident: {
                        let __read = ::std::clone::Clone::clone(&shared);
                        let __write = ::std::clone::Clone::clone(&shared);
                        <::xplm::dataref::PublishedDataReadWrite>::publish_writable(
                            #path_lit,
                            move || ::std::clone::Clone::clone(&(*__read.borrow()).#field_ident),
                            move |bytes: &[u8]| (*__write.borrow_mut()).#field_ident = bytes.to_vec(),
                        )?
                    }
                }
            } else {
                quote! {
                    #field_ident: {
                        let __read = ::std::clone::Clone::clone(&shared);
                        <::xplm::dataref::PublishedDataReadOnly>::publish(
                            #path_lit,
                            move || ::std::clone::Clone::clone(&(*__read.borrow()).#field_ident),
                        )?
                    }
                }
            });
        } else if let Some(elem_ty) = vec_array_elem_type(field_ty) {
            let handle_ty = if writable {
                quote!(::xplm::dataref::PublishedArrayReadWrite<#elem_ty>)
            } else {
                quote!(::xplm::dataref::PublishedArrayReadOnly<#elem_ty>)
            };
            registration_fields.push(quote!(#field_ident: #handle_ty));

            accessor_methods.push(quote! {
                #vis fn #field_ident(&self) -> #field_ty {
                    ::std::clone::Clone::clone(&(*self.state.borrow()).#field_ident)
                }
            });
            if writable {
                accessor_methods.push(quote! {
                    #vis fn #set_ident(&self, value: #field_ty) {
                        (*self.state.borrow_mut()).#field_ident = value;
                    }
                });
            }

            publish_inits.push(if writable {
                quote! {
                    #field_ident: {
                        let __read = ::std::clone::Clone::clone(&shared);
                        let __write = ::std::clone::Clone::clone(&shared);
                        <#handle_ty>::publish_writable(
                            #path_lit,
                            move || ::std::clone::Clone::clone(&(*__read.borrow()).#field_ident),
                            move |offset: usize, values: &[#elem_ty]| {
                                let mut state = __write.borrow_mut();
                                let vec = &mut state.#field_ident;
                                let end = offset + values.len();
                                if vec.len() < end {
                                    vec.resize(end, ::std::default::Default::default());
                                }
                                vec[offset..end].copy_from_slice(values);
                            },
                        )?
                    }
                }
            } else {
                quote! {
                    #field_ident: {
                        let __read = ::std::clone::Clone::clone(&shared);
                        <#handle_ty>::publish(
                            #path_lit,
                            move || ::std::clone::Clone::clone(&(*__read.borrow()).#field_ident),
                        )?
                    }
                }
            });
        } else {
            // A #[packed] field (T: PackedDataRef, asserted by the caller —
            // see xplm::dataref::PackedDataRef) registers through
            // PublishedStruct instead of PublishedDataRef. The accessor
            // methods below are identical either way: they only ever touch
            // `self.state` (this plugin's own in-memory value), never the
            // registration itself, so there's nothing to reconcile between
            // the two — only which type gets registered differs.
            let handle_ty = match (packed, writable) {
                (true, true) => quote!(::xplm::dataref::PublishedStructReadWrite<#field_ty>),
                (true, false) => quote!(::xplm::dataref::PublishedStructReadOnly<#field_ty>),
                (false, true) => quote!(::xplm::dataref::PublishedReadWrite<#field_ty>),
                (false, false) => quote!(::xplm::dataref::PublishedReadOnly<#field_ty>),
            };
            registration_fields.push(quote!(#field_ident: #handle_ty));

            accessor_methods.push(quote! {
                #vis fn #field_ident(&self) -> #field_ty {
                    (*self.state.borrow()).#field_ident
                }
            });
            if writable {
                accessor_methods.push(quote! {
                    #vis fn #set_ident(&self, value: #field_ty) {
                        (*self.state.borrow_mut()).#field_ident = value;
                    }
                });
            }

            publish_inits.push(if writable {
                quote! {
                    #field_ident: {
                        let __read = ::std::clone::Clone::clone(&shared);
                        let __write = ::std::clone::Clone::clone(&shared);
                        <#handle_ty>::publish_writable(
                            #path_lit,
                            move || (*__read.borrow()).#field_ident,
                            move |v| (*__write.borrow_mut()).#field_ident = v,
                        )?
                    }
                }
            } else {
                quote! {
                    #field_ident: {
                        let __read = ::std::clone::Clone::clone(&shared);
                        <#handle_ty>::publish(
                            #path_lit,
                            move || (*__read.borrow()).#field_ident,
                        )?
                    }
                }
            });
        }
    }

    quote! {
        // Not part of the public API: purely storage for the RAII
        // registrations backing a #handle_ident, kept alive by (and
        // unregistered once the last clone of) that Handle — see
        // `#[derive(PublishedDataRefContainer)]`'s doc comment.
        #[doc(hidden)]
        #vis struct #registrations_ident {
            #(#registration_fields),*
        }

        /// Cheap-to-`Clone` accessor over the shared state `publish` wraps
        /// its receiver in — one getter (and, for `#[writable]` fields, one
        /// `set_*` setter) per field, so call sites don't repeat
        /// `.borrow()`/`.borrow_mut()`. Every published dataref stays
        /// registered for as long as *any* clone of a given `publish()`
        /// call's `Handle` is alive, and unregisters automatically once the
        /// last one drops — there's nothing else to hold onto or drop by
        /// hand. See `#[derive(PublishedDataRefContainer)]`'s doc comment.
        #vis struct #handle_ident {
            state: ::std::rc::Rc<::std::cell::RefCell<#ident>>,
            _registrations: ::std::rc::Rc<#registrations_ident>,
        }

        impl ::std::clone::Clone for #handle_ident {
            fn clone(&self) -> Self {
                Self {
                    state: ::std::rc::Rc::clone(&self.state),
                    _registrations: ::std::rc::Rc::clone(&self._registrations),
                }
            }
        }

        impl #handle_ident {
            #(#accessor_methods)*
        }

        impl #ident {
            /// Wraps `self` in a shared handle and registers every
            /// `#[dataref = "..."]` field on it, returning a single
            /// `Handle` for reading/writing fields directly — see
            /// `#[derive(PublishedDataRefContainer)]`'s doc comment.
            #vis fn publish(self) -> ::std::option::Option<#handle_ident> {
                let shared = ::std::rc::Rc::new(::std::cell::RefCell::new(self));
                let registrations = #registrations_ident {
                    #(#publish_inits),*
                };
                ::std::option::Option::Some(#handle_ident {
                    state: shared,
                    _registrations: ::std::rc::Rc::new(registrations),
                })
            }
        }
    }
    .into()
}

/// Whether `ty` is exactly `Vec<u8>` — the marker this derive uses to
/// dispatch a field to [`xplm::dataref::PublishedData`] (`xplmType_Data`)
/// instead of [`xplm::dataref::PublishedDataRef`]'s scalar path.
fn is_vec_u8(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return false;
    };
    if segment.ident != "Vec" {
        return false;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };
    matches!(
        args.args.first(),
        Some(GenericArgument::Type(Type::Path(inner))) if inner.path.is_ident("u8")
    )
}

/// Whether `ty` is exactly `Vec<i32>`/`Vec<f32>` (not `Vec<u8>`, which
/// [`is_vec_u8`] already claims) — the marker this derive uses to dispatch a
/// field to [`xplm::dataref::PublishedArray`]/[`xplm::dataref::ArrayDataRef`]
/// instead of the byte-string or scalar path. Returns the element type
/// (`i32` or `f32`) on a match.
fn vec_array_elem_type(ty: &Type) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Vec" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(inner @ Type::Path(inner_path))) = args.args.first() else {
        return None;
    };
    (inner_path.path.is_ident("i32") || inner_path.path.is_ident("f32")).then(|| inner.clone())
}

/// Whether `ty` is a bare `i32`/`f32`/`f64`/`Vec<u8>`/`Vec<i32>`/`Vec<f32>` —
/// the marker `#[derive(DataRefContainer)]` uses to switch a field from
/// "stored as the wrapper type you wrote" to "generate a `Handle`
/// getter/setter for the plain value instead".
fn is_plain_type(ty: &Type) -> bool {
    if is_vec_u8(ty) || vec_array_elem_type(ty).is_some() {
        return true;
    }
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path.qself.is_none()
        && type_path.path.segments.len() == 1
        && matches!(
            type_path.path.segments[0].ident.to_string().as_str(),
            "i32" | "f32" | "f64"
        )
        && matches!(type_path.path.segments[0].arguments, PathArguments::None)
}

/// Whether `field` should go through `#[derive(DataRefContainer)]`'s/
/// `#[derive(PublishedDataRefContainer)]`'s plain-typed handling — either
/// its type is a bare `i32`/`f32`/`f64`/`Vec<u8>` ([`is_plain_type`]), or
/// it's tagged `#[packed]` (an arbitrary `T: PackedDataRef`, asserted by the
/// caller — macros can't check trait bounds at expansion time, so `#[packed]`
/// is how you opt a field into that treatment for a type the derive can't
/// otherwise recognize as plain).
fn field_is_plain(field: &syn::Field) -> bool {
    is_plain_type(&field.ty)
        || field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("packed"))
}

/// A recognized wrapper type in a `#[derive(DataRefContainer)]` field mixed
/// alongside plain-typed fields — the only kinds with a well-defined plain
/// value to put in the generated Snapshot type.
enum WrapperKind {
    /// `ReadOnly<T>`/`ReadWrite<T>`, with `T` and whether it was `ReadWrite`.
    Scalar { inner: Type, writable: bool },
    /// `ReadOnlyBytes`/`ReadWriteBytes` (no generic parameter).
    Bytes { writable: bool },
    /// `ReadOnlyStruct<T>`/`ReadWriteStruct<T>` (`T: PackedDataRef`), with
    /// `T` and whether it was `ReadWriteStruct`.
    Packed { inner: Type, writable: bool },
    /// `ReadOnlyArray<T>`/`ReadWriteArray<T>` (`T: i32`/`f32`), with `T` and
    /// whether it was `ReadWriteArray`.
    Array { inner: Type, writable: bool },
}

/// Recognizes `ty` as `ReadOnly<T>`/`ReadWrite<T>`/`ReadOnlyBytes`/
/// `ReadWriteBytes`/`ReadOnlyStruct<T>`/`ReadWriteStruct<T>`/
/// `ReadOnlyArray<T>`/`ReadWriteArray<T>` specifically (by their exact
/// written name — this is a syntactic check, not a type-level one, same
/// limitation `is_vec_u8` has). Returns `None` for anything else (a raw
/// `DataRef<T, A>`/`ArrayDataRef<T, A>`, plain types, unrelated types).
fn classify_wrapper(ty: &Type) -> Option<WrapperKind> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    match segment.ident.to_string().as_str() {
        "ReadOnly" | "ReadWrite" => {
            let PathArguments::AngleBracketed(args) = &segment.arguments else {
                return None;
            };
            let GenericArgument::Type(inner) = args.args.first()? else {
                return None;
            };
            Some(WrapperKind::Scalar {
                inner: inner.clone(),
                writable: segment.ident == "ReadWrite",
            })
        }
        "ReadOnlyBytes" | "ReadWriteBytes" => Some(WrapperKind::Bytes {
            writable: segment.ident == "ReadWriteBytes",
        }),
        "ReadOnlyStruct" | "ReadWriteStruct" => {
            let PathArguments::AngleBracketed(args) = &segment.arguments else {
                return None;
            };
            let GenericArgument::Type(inner) = args.args.first()? else {
                return None;
            };
            Some(WrapperKind::Packed {
                inner: inner.clone(),
                writable: segment.ident == "ReadWriteStruct",
            })
        }
        "ReadOnlyArray" | "ReadWriteArray" => {
            let PathArguments::AngleBracketed(args) = &segment.arguments else {
                return None;
            };
            let GenericArgument::Type(inner) = args.args.first()? else {
                return None;
            };
            Some(WrapperKind::Array {
                inner: inner.clone(),
                writable: segment.ident == "ReadWriteArray",
            })
        }
        _ => None,
    }
}

/// Parses an optional struct-level `#[dataref_prefix = "..."]` out of
/// `attrs`, shared by `#[derive(DataRefContainer)]`'s plain-typed style and
/// `#[derive(PublishedDataRefContainer)]`. Returns `""` if the attribute is
/// absent — the caller then requires every field to specify its own
/// `#[dataref = "..."]` instead of falling back to `prefix + field name`.
fn dataref_prefix_from_attrs(attrs: &[syn::Attribute]) -> syn::Result<String> {
    let prefix_attrs: Vec<_> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("dataref_prefix"))
        .collect();
    match prefix_attrs.as_slice() {
        [] => Ok(String::new()),
        [attr] => path_lit_from_attr(attr).map(|lit| lit.value()),
        [_, extra, ..] => Err(syn::Error::new_spanned(
            extra,
            "only one #[dataref_prefix = \"...\"] allowed",
        )),
    }
}

fn path_lit_from_attr(attr: &syn::Attribute) -> syn::Result<LitStr> {
    let Meta::NameValue(nv) = &attr.meta else {
        return Err(syn::Error::new_spanned(
            attr,
            "expected `#[dataref = \"sim/...\"]`",
        ));
    };
    let Expr::Lit(expr_lit) = &nv.value else {
        return Err(syn::Error::new_spanned(
            &nv.value,
            "expected a string literal",
        ));
    };
    let Lit::Str(s) = &expr_lit.lit else {
        return Err(syn::Error::new_spanned(
            &expr_lit.lit,
            "expected a string literal",
        ));
    };
    Ok(s.clone())
}
