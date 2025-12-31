use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, Lit, Expr};

#[proc_macro_attribute]
pub fn retry(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Support either empty `#[retry]` (defaults to 3) or a single integer `#[retry(3)]`.
    let mut attempts: Option<usize> = None;
    if !attr.is_empty() {
        match syn::parse::<Expr>(attr.clone()) {
            Ok(Expr::Lit(syn::ExprLit { lit: Lit::Int(litint), .. })) => {
                match litint.base10_parse::<usize>() {
                    Ok(n) => attempts = Some(n),
                    Err(_) => return syn::Error::new_spanned(litint, "invalid integer").to_compile_error().into(),
                }
            }
            Ok(_) => return syn::Error::new(proc_macro2::Span::call_site(), "unsupported attribute form; expected `N` or empty").to_compile_error().into(),
            Err(_) => return syn::Error::new(proc_macro2::Span::call_site(), "failed to parse attribute; expected integer literal like `#[retry(3)]`").to_compile_error().into(),
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
    let mut sig = input.sig.clone();
    let attrs = &input.attrs;
    let block = &input.block;

    // Collect simple parameter identifiers to clone inside the closure per-attempt
    let mut clones = Vec::new();
    for input in sig.inputs.iter() {
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let ident = &pat_ident.ident;
                clones.push(quote::quote! { let #ident = #ident.clone(); });
            }
        }
    }

    // Build the new function body that wraps the original body inside a RetryPolicy::retry call
    // We'll reference the runtime crate as `::asyn_retry_policy::RetryPolicy`

    let expanded = quote! {
        #(#attrs)*
        #vis #sig {
            let policy = ::asyn_retry_policy::RetryPolicy { attempts: #attempts, ..Default::default() };
            policy.retry(|| {
                #(#clones)*
                async move #block
            }, |_| true).await
        }
    };

    expanded.into()
}
