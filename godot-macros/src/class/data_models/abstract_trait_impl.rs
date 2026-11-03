/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use proc_macro2::TokenStream;
use quote::quote;

use crate::class::{FuncDefinition, into_signature_info, make_method_registration};
use crate::util::{bail, ident};
use crate::{ParseResult, util};

/// Returns true if `#[godot_api]` was applied to an `impl I<UserClass> for Subclass` block.
///
/// Both engine interface traits and the traits generated for abstract user classes are named `I<Class>`; they are told apart by whether
/// `<Class>` is a Godot engine class.
pub fn is_abstract_trait_impl(impl_block: &venial::Impl) -> bool {
    let Some(trait_ty) = impl_block.trait_ty.as_ref() else {
        return false;
    };

    util::extract_typename(trait_ty).is_some_and(|segment| {
        let name = segment.ident.to_string();
        match name.strip_prefix('I') {
            Some(class_name) => !class_name.is_empty() && !util::is_engine_class(class_name),
            None => false,
        }
    })
}

/// Codegen for `#[godot_api] impl IMyAbstractClass for MySubclass`.
///
/// Registers every method of the trait implementation with Godot, under the *subclass*. The abstract base class only registers signature
/// metadata (see `make_abstract_method_registration()`); each concrete subclass needs its own callable method, dispatching to its own
/// instance.
pub fn transform_abstract_trait_impl(impl_block: venial::Impl) -> ParseResult<TokenStream> {
    let class_name = util::validate_impl(&impl_block, None, "godot_api")?;
    let trait_ty = impl_block
        .trait_ty
        .clone()
        .expect("checked by is_abstract_trait_impl");

    let mut method_registrations = Vec::new();

    for item in impl_block.body_items.iter() {
        match item {
            venial::ImplMember::AssocFunction(function) => {
                if function.generic_params.is_some() {
                    return bail!(
                        &function.generic_params,
                        "#[godot_api]: generic fn parameters are not supported",
                    );
                }

                if function.body.is_none() {
                    return bail!(
                        function,
                        "#[godot_api]: abstract trait implementations must provide a body for every method",
                    );
                }

                if let Some(attr) = crate::util::KvParser::parse(&function.attributes, "func")? {
                    return bail!(
                        attr.span(),
                        "#[func] is not needed in an abstract trait impl; all methods are registered",
                    );
                }

                let external_attributes = function.attributes.clone();
                let signature = util::reduce_to_signature(function);
                let signature_info = into_signature_info(signature, &class_name, false);

                method_registrations.push(make_method_registration(
                    &class_name,
                    FuncDefinition {
                        signature_info,
                        external_attributes,
                        registered_name: None,
                        is_script_virtual: false,
                        is_abstract: false,
                        rpc_info: None,
                        is_generated_accessor: false,
                    },
                    Some(&trait_ty),
                )?);
            }

            venial::ImplMember::AssocConstant(constant) => {
                return bail!(
                    constant,
                    "#[godot_api]: associated constants are not supported in abstract trait impls",
                );
            }

            _ => {}
        }
    }

    // Reuse the registration storage set up by the class's primary `#[godot_api] impl` block -- same mechanism as secondary impl blocks.
    let class_name = ident(&class_name.to_string());

    // An abstract class has no instances, so a method registered for it would be invoked with a *subclass* instance of a different Rust
    // type. The methods also would not reach the subclass, since only a class's own `#[godot_api]` block contributes inheritable members.
    let deny_abstract_self = quote! {
        const _: () = assert!(
            !<#class_name as ::godot::obj::GodotClass>::__IS_ABSTRACT,
            "an abstract class cannot implement another abstract class's `I...` trait; \
             declare a concrete #[func] of the same name instead, which subclasses inherit",
        );
    };
    let fill_storage = if method_registrations.is_empty() {
        TokenStream::new()
    } else {
        quote! {
            ::godot::sys::shard_execute_pre_main!({
                let mut guard = #class_name::__registration_storage().lock().unwrap();

                guard.0.push(|| {
                    let __godot_class_id = <#class_name as ::godot::obj::GodotClass>::class_id();
                    #( #method_registrations )*
                });
            });
        }
    };

    Ok(quote! {
        #impl_block
        #deny_abstract_self
        #fill_storage
    })
}
