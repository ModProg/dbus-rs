#![allow(unused)]

use std::{
    iter::{self, Peekable},
    str::CharIndices,
};

use attribute_derive::parsing::{AttributeBase, AttributeValue, SpannedValue};
use manyhow::{bail, ensure, error_message, ErrorMessage, Result};
use proc_macro2::{Span, TokenStream};
use quote_use::{quote_spanned_use, quote_use, ToTokens};
use syn::{custom_keyword, parse::Parse, LitStr, Token};

#[derive(Debug)]
pub struct Signature {
    pub src: LitStr,
    pub parsed: Vec<DbusType>,
}

impl Signature {
    pub fn expand_to_signature(&self) -> TokenStream {
        let mut signature_lit = self.src.value();

        if !signature_lit.ends_with('\0') {
            signature_lit.push('\0');
        }
        quote_spanned_use! {self.src.span()=>
            // SAFETY: \0 is appended
            unsafe { ::dbus::strings::Signature::from_slice_unchecked(#signature_lit) }
        }
    }
}

impl ToTokens for Signature {
    fn to_tokens(&self, tokens: &mut TokenStream) { self.src.to_tokens(tokens); }
}

impl Parse for Signature {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let src: LitStr = input.parse()?;

        let s = src.value();
        ensure!(s.len() <= 255, "maximum length of signature is 255");
        let mut s = s.char_indices().peekable();
        let parsed = iter::from_fn(|| {
            if let Some((i, '\0')) = s.peek().copied() {
                s.next();
                if s.peek().is_some() {
                    Some(error_message!(src, "expected end of input after `\0` at character {i}"));
                }
                None
            } else {
                DbusType::parse(&mut s, src.span()).transpose()
            }
        })
        .collect::<Result<_, _>>()?;
        // we could do further validation here i.e. max 32 levels of struct and array nesting each
        Ok(Self { parsed, src })
    }
}

attribute_derive::impl_Attribute_for_Parse_and_ToTokens!(Signature);

#[derive(Debug)]
pub enum SimpleType {
    /// `y`
    Byte,
    /// `b`
    Bool,
    /// `n`
    Int16,
    /// `q`
    UInt16,
    /// `i`
    Int32,
    /// `u`
    UInt32,
    /// `d`
    Double,
    /// `h`
    UnixFd,
    /// `s`
    String,
    /// `o`
    ObjectPath,
    /// `g`
    Signature,
}

impl SimpleType {
    fn from_char(c: char, idx: usize, span: Span) -> Result<Self, ErrorMessage> {
        Ok(match c {
            'y' => Self::Byte,
            'b' => Self::Bool,
            'n' => Self::Int16,
            'q' => Self::UInt16,
            'i' => Self::Int32,
            'u' => Self::UInt32,
            'd' => Self::Double,
            'h' => Self::UnixFd,
            's' => Self::String,
            'o' => Self::ObjectPath,
            'g' => Self::Signature,
            o => bail!(span, "got `{o}` but expected a simple type at character {idx}, i.e., one of: y, b, n, q, i, u, d, h, s, o, g"),
        })
    }
}

#[derive(Debug)]
pub enum DbusType {
    Simple(SimpleType),
    /// `( <...> )`
    Struct(Vec<DbusType>),
    /// `a`
    Array(Box<DbusType>),
    /// `v`
    Variant,
    /// `a{ <...> }`
    Dict(SimpleType, Box<DbusType>),
}

impl DbusType {
    fn parse(s: &mut Peekable<CharIndices>, span: Span) -> syn::Result<Option<Self>> {
        if let Some((i, c)) = s.next() {
            Ok(Some(match c {
                'y' => Self::Simple(SimpleType::Byte),
                'b' => Self::Simple(SimpleType::Bool),
                'n' => Self::Simple(SimpleType::Int16),
                'q' => Self::Simple(SimpleType::UInt16),
                'i' => Self::Simple(SimpleType::Int32),
                'u' => Self::Simple(SimpleType::UInt32),
                'd' => Self::Simple(SimpleType::Double),
                'h' => Self::Simple(SimpleType::UnixFd),
                's' => Self::Simple(SimpleType::String),
                'o' => Self::Simple(SimpleType::ObjectPath),
                'g' => Self::Simple(SimpleType::Signature),
                '(' => {
                    let types = iter::from_fn(|| if matches!(s.peek(), Some((_, ')'))) { None } else { Self::parse(s, span).transpose() })
                        .collect::<Result<_, _>>()?;
                    ensure!(s.next().is_some_and(|c| c.1 == ')'), span, "paren at character {i} is not closed");
                    Self::Struct(types)
                }
                'a' if matches!(s.peek(), Some((_, '{'))) => {
                    let i = s.next().unwrap(/*just peeked*/).0;
                    let (ki, kc) =
                        s.next().ok_or_else(|| error_message!(span, "expected key type for the dict entry starting at character {i}"))?;
                    let key = SimpleType::from_char(kc, ki, span)?;
                    let value = Self::parse(s, span)?
                        .ok_or_else(|| error_message!(span, "expected the value type for the dict entry starting at character {i}"))?;
                    Self::Dict(key, Box::new(value))
                }
                'a' => Self::Array(Box::new(
                    Self::parse(s, span)?.ok_or_else(|| error_message!(span, "missing array type at character {}", i + 1))?,
                )),
                'v' => Self::Variant,
                o => bail!(
                    span,
                    "got `{o}` but expected a dbus type at character {i}, i.e., one of: y, b, n, q, i, u, d, h, s, o, g, (, a, v"
                ),
            }))
        } else {
            Ok(None)
        }
    }
}
