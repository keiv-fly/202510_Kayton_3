use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, punctuated::Punctuated, Expr, ExprLit, FnArg, ItemFn, Lit, Meta, Pat,
    PatIdent, Token, Type,
};

struct ExtensionAttr {
    name: String,
    doc: Option<String>,
}

impl ExtensionAttr {
    fn from_args(args: Punctuated<Meta, Token![,]>) -> syn::Result<Self> {
        let mut name = None;
        let mut doc = None;
        for meta in args {
            match meta {
                Meta::NameValue(nv) if nv.path.is_ident("name") => {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(lit), ..
                    }) = nv.value
                    {
                        name = Some(lit.value());
                    } else {
                        return Err(syn::Error::new_spanned(
                            nv.value,
                            "name must be a string literal",
                        ));
                    }
                }
                Meta::NameValue(nv) if nv.path.is_ident("doc") => {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(lit), ..
                    }) = nv.value
                    {
                        doc = Some(lit.value());
                    } else {
                        return Err(syn::Error::new_spanned(
                            nv.value,
                            "doc must be a string literal",
                        ));
                    }
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "unsupported attribute parameter",
                    ));
                }
            }
        }
        let name = name.ok_or_else(|| {
            syn::Error::new(Span::call_site(), "expected attribute `name = \"value\"`")
        })?;
        Ok(Self { name, doc })
    }
}

#[proc_macro_attribute]
pub fn kayton_extension(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, Token![,]>::parse_terminated);
    let attr = match ExtensionAttr::from_args(args) {
        Ok(attr) => attr,
        Err(err) => return err.into_compile_error().into(),
    };

    let func = parse_macro_input!(item as ItemFn);
    match expand_extension(attr, func) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

fn expand_extension(attr: ExtensionAttr, func: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let fn_name = func.sig.ident.clone();
    let adapter_name = format_ident!("__kayton_adapter_{}", fn_name);
    let const_name = Ident::new(
        &format!("{}_EXTENSION", fn_name.to_string().to_uppercase()),
        Span::call_site(),
    );

    if !func.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &func.sig.generics,
            "generics are not supported",
        ));
    }

    if func.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            func.sig.fn_token,
            "async functions are not supported",
        ));
    }

    let inputs = func.sig.inputs.iter().collect::<Vec<_>>();
    if inputs.is_empty() {
        return Err(syn::Error::new_spanned(
            &func.sig,
            "expected at least `ctx: &KayCtx` argument",
        ));
    }

    let mut args_iter = inputs.into_iter();
    let ctx_arg = args_iter
        .next()
        .ok_or_else(|| syn::Error::new_spanned(&func.sig, "missing ctx argument"))?;
    ensure_ctx_arg(ctx_arg)?;

    let mut arg_bindings = Vec::new();
    for (idx, arg) in args_iter.enumerate() {
        match arg {
            FnArg::Typed(pat_type) => {
                let pat = match pat_type.pat.as_ref() {
                    Pat::Ident(PatIdent { ident, .. }) => ident.clone(),
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &pat_type.pat,
                            "arguments must be simple identifiers",
                        ));
                    }
                };
                let ty = (*pat_type.ty).clone();
                arg_bindings.push((pat, ty, idx));
            }
            FnArg::Receiver(recv) => {
                return Err(syn::Error::new_spanned(recv, "methods are not supported"));
            }
        }
    }

    let arity = arg_bindings.len();
    let arg_names: Vec<_> = arg_bindings.iter().map(|(pat, _, _)| pat).collect();
    let arg_types: Vec<_> = arg_bindings.iter().map(|(_, ty, _)| ty).collect();
    let arg_indices: Vec<_> = arg_bindings.iter().map(|(_, _, idx)| idx).collect();

    let doc = attr.doc.unwrap_or_default();
    let doc_lit = Lit::Str(syn::LitStr::new(&doc, Span::call_site()));
    let name_lit = syn::LitStr::new(&attr.name, Span::call_site());
    let arity_literal = syn::LitInt::new(&arity.to_string(), Span::call_site());

    let adapter = quote! {
        fn #adapter_name(ctx: &kayton_api::KayCtx, args: &[kayton_api::KayHandle]) -> kayton_api::KayResult<kayton_api::KayHandle> {
            if args.len() != #arity_literal {
                return Err(kayton_api::KayError::new(
                    kayton_api::KayErrorCode::InvalidArgument,
                    format!("expected {} arguments, found {}", #arity_literal, args.len()),
                ));
            }
            #(let #arg_names: #arg_types = kayton_api::FromKay::from_kay(ctx, &args[#arg_indices])?;)*
            let value = #fn_name(ctx, #( #arg_names ),* )?;
            kayton_api::ToKay::to_kay(value, ctx)
        }
    };

    let const_decl = quote! {
        pub const #const_name: kayton_api::KayExtension = kayton_api::KayExtension::new(
            #name_lit,
            #adapter_name,
            #arity_literal,
            Some(#arity_literal),
            #doc_lit,
        );
    };

    Ok(quote! {
        #func
        #adapter
        #const_decl
    })
}

fn ensure_ctx_arg(arg: &FnArg) -> syn::Result<()> {
    match arg {
        FnArg::Typed(pat_type) => {
            if let Type::Reference(reference) = pat_type.ty.as_ref() {
                if let Type::Path(path) = reference.elem.as_ref() {
                    if path.path.is_ident("KayCtx") || path.path.is_ident("kayton_api::KayCtx") {
                        return Ok(());
                    }
                }
            }
            Err(syn::Error::new_spanned(
                &pat_type.ty,
                "first argument must be `&KayCtx`",
            ))
        }
        FnArg::Receiver(_) => Err(syn::Error::new_spanned(arg, "methods are not supported")),
    }
}
