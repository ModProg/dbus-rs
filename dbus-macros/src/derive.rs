use attribute_derive::FromAttr;
use manyhow::{bail, ensure, Result};
use quote_use::{format_ident, quote_use as quote, quote_use_no_prelude};
use syn::parse_quote;
use syn::{spanned::Spanned, Data, DataStruct, DeriveInput, Fields, Ident};

use crate::signature::{DbusType, SimpleType};
use crate::Dbus;

pub fn arg(DeriveInput { attrs, ident, mut generics, data, .. }: DeriveInput) -> Result {
    let Dbus { signature, as_struct } = Dbus::from_attributes(&attrs)?;

    if let Some(signature) = signature {
        ensure!(signature.parsed.len() == 1, signature.span(), "expected one type");
        let arg_type = signature.parsed[0].arg_type();
        let signature = signature.expand_to_signature();

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        Ok(quote! {
            # use dbus::{arg, strings};
            #[automatically_derived]
            impl #impl_generics arg::Arg for #ident #ty_generics #where_clause {
                const ARG_TYPE: arg::ArgType = arg::ArgType::#arg_type;

                fn signature() -> strings::Signature<'static> {
                    #signature
                }
            }
        })
    } else {
        let arg_type;
        let signature;
        match data {
            Data::Struct(DataStruct { fields, .. }) if matches!(fields, Fields::Unnamed(_)) || as_struct => {
                let fields = fields.into_iter().map(|f| f.ty);
                arg_type = quote!(Struct);
                signature = quote! {
                    # use dbus::{arg, strings};
                    let mut __signature = String::from("(");
                    #(__signature.push_str(&*<#fields as arg::Arg>::signature());)*
                    __signature.push(')');
                    strings::Signature::new(__signature).unwrap(/*valid signatures inside struct should be valid signature*/)
                };
            }
            Data::Struct(DataStruct { fields: Fields::Unnamed(_), .. }) => unreachable!(),
            Data::Struct(DataStruct { fields: Fields::Named(_), .. }) => {
                arg_type = quote!(Array);
                signature = quote! {
                    // SAFETY: has trailing \0 and `a{sv}` is a valid signature
                    unsafe { ::dbus::strings::Signature::from_slice_unchecked("a{sv}\0") }
                };
            }
            Data::Struct(DataStruct { fields: Fields::Unit, .. }) => {
                bail!(ident, "cannot infer signature for unit structs"; help="specify `#[signature=\"dbus-signature\"]`")
            }
            Data::Enum(data) if data.variants.is_empty() => {
                bail!(data.brace_token.span.span(), "cannot infer signature for enums without variants"; help="specify manually `#[signature=\"dbus-signature\"]`")
            }
            Data::Enum(data) if data.variants.iter().all(|v| v.fields.is_empty()) => {
                // TODO should we consider the `#[repr]` if one is specified for an enum?
                arg_type = quote_use_no_prelude!(String);
                signature = quote! {
                    // SAFETY: has trailing \0 and `s` is a valid signature
                    unsafe { ::dbus::strings::Signature::from_slice_unchecked("s\0") }
                };
            }
            Data::Enum(data) => {
                let variant =
                    data.variants.iter().find(|v| !v.fields.is_empty()).unwrap(/*we only get here when there is a non empty variant*/);
                bail!(variant.fields, "enums with fields are not yet supported"; help="specify manually, e.g. `#[signature=\"v\"]`")
            }
            Data::Union(data) => {
                bail!(data.union_token, "cannot infer signature for unions"; help="specify manually `#[signature=\"dbus-signature\"]`")
            }
        }
        for generic in generics.type_params_mut() {
            generic.bounds.push(parse_quote!(::dbus::arg::Arg))
        }
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        Ok(quote! {
            # use dbus::{arg, strings};
            #[automatically_derived]
            impl #impl_generics arg::Arg for #ident #ty_generics #where_clause {
                const ARG_TYPE: arg::ArgType = arg::ArgType::#arg_type;

                fn signature() -> strings::Signature<'static> {
                    #signature
                }
            }
        })
    }
}

impl DbusType {
    fn arg_type(&self) -> Ident {
        match self {
            DbusType::Simple(s) => s.arg_type(),
            DbusType::Struct(_) => format_ident!("Struct"),
            DbusType::Array(_) | DbusType::Dict(_, _) => format_ident!("Array"),
            DbusType::Variant => format_ident!("Variant"),
        }
    }
}

impl SimpleType {
    fn arg_type(&self) -> Ident {
        match self {
            SimpleType::Byte => format_ident!("Byte"),
            SimpleType::Bool => format_ident!("Bool"),
            SimpleType::Int16 => format_ident!("Int16"),
            SimpleType::UInt16 => format_ident!("UInt16"),
            SimpleType::Int32 => format_ident!("Int32"),
            SimpleType::UInt32 => format_ident!("UInt32"),
            SimpleType::Double => format_ident!("Double"),
            SimpleType::UnixFd => format_ident!("UnixFd"),
            SimpleType::String => format_ident!("String"),
            SimpleType::ObjectPath => format_ident!("ObjectPath"),
            SimpleType::Signature => format_ident!("Signature"),
        }
    }
}
