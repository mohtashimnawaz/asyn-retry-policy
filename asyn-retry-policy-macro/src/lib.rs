use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, AttributeArgs, ItemFn, NestedMeta, Meta, Lit};

#[proc_macro_attribute]
pub fn retry(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute arguments and the function
    let args = parse_macro_input!(attr as AttributeArgs);
    let mut attempts: Option<usize> = None;

    for meta in args.iter() {
        match meta {
            NestedMeta::Lit(Lit::Int(litint)) => {
                if attempts.is_some() {
                    return syn::Error::new_spanned(meta, "duplicate attempts").to_compile_error().into();
                }
                match litint.base10_parse::<usize>() {
                    Ok(n) => attempts = Some(n),
                    Err(_) => return syn::Error::new_spanned(litint, "invalid integer").to_compile_error().into(),
                }
            }
            NestedMeta::Meta(Meta::NameValue(nv)) if nv.path.is_ident("attempts") => {
                match &nv.lit {
                    Lit::Int(litint) => match litint.base10_parse::<usize>() {
                        Ok(n) => attempts = Some(n),
                        Err(_) => return syn::Error::new_spanned(nv, "invalid integer").to_compile_error().into(),
                    },
                    _ => return syn::Error::new_spanned(nv, "expected integer literal").to_compile_error().into(),
                }
            }
            other => return syn::Error::new_spanned(other, "unsupported argument; expected `N` or `attempts = N`").to_compile_error().into(),
        }
    }

    // Default attempts if not provided
    let attempts = attempts.unwrap_or(3usize);

    let input = parse_macro_input!(item as ItemFn);

    // Ensure function is async
    if input.sig.asyncness.is_none() {
        return syn::Error::new_spanned(input.sig.fn_token, "`#[retry]` can only be applied to `async fn`").to_compile_error().into();
    }

    let vis = &input.vis;
    let sig = &input.sig;
    let attrs = &input.attrs;
    let block = &input.block;

    // Build the new function body that wraps the original body inside a RetryPolicy::retry call
    // We'll reference the runtime crate as `::asyn_retry_policy::RetryPolicy`

    let gen = quote! {
        #(#attrs)*
        #vis #sig {
            let policy = ::asyn_retry_policy::RetryPolicy { attempts: #attempts, ..Default::default() };
            policy.retry(|| async move #block, |_| true).await
        }
    };

    gen.into()
}
