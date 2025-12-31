use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, Lit, Expr};

#[proc_macro_attribute]
pub fn retry(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Supported attribute forms:
    // - empty: `#[retry]`
    // - single integer: `#[retry(3)]`
    // - named args: `#[retry(attempts = 3, base_delay_ms = 100, max_delay_ms = 5000, backoff_factor = 2.0, jitter = true, rng_seed = 42)]`

    let mut attempts: Option<usize> = None;
    let mut base_delay_ms: Option<u64> = None;
    let mut max_delay_ms: Option<u64> = None;
    let mut backoff_factor: Option<f64> = None;
    let mut jitter_opt: Option<bool> = None;
    let mut rng_seed: Option<u64> = None;
    let mut predicate_expr: Option<syn::Expr> = None;

    if !attr.is_empty() {
        // try simple integer form first
        if let Ok(Expr::Lit(syn::ExprLit { lit: Lit::Int(litint), .. })) = syn::parse::<Expr>(attr.clone()) {
            attempts = Some(litint.base10_parse::<usize>().unwrap_or(3));
        } else {
            // parse named args using a simple key = expr parser
            struct KeyVals(Vec<(syn::Ident, syn::Expr)>);

            impl syn::parse::Parse for KeyVals {
                fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
                    let mut out = Vec::new();
                    while !input.is_empty() {
                        let key: syn::Ident = input.parse()?;
                        input.parse::<syn::Token![=]>()?;
                        let expr: syn::Expr = input.parse()?;
                        out.push((key, expr));
                        if input.peek(syn::Token![,]) {
                            let _ = input.parse::<syn::Token![,]>()?;
                        }
                    }
                    Ok(KeyVals(out))
                }
            }

            let args = parse_macro_input!(attr as KeyVals);
            for (ident, expr) in args.0 {
                match ident.to_string().as_str() {
                    "attempts" => match expr {
                        Expr::Lit(syn::ExprLit { lit: Lit::Int(litint), .. }) => attempts = Some(litint.base10_parse::<usize>().unwrap()),
                        _ => return syn::Error::new_spanned(expr, "expected integer literal").to_compile_error().into(),
                    },
                    "base_delay_ms" => match expr {
                        Expr::Lit(syn::ExprLit { lit: Lit::Int(litint), .. }) => base_delay_ms = Some(litint.base10_parse::<u64>().unwrap()),
                        _ => return syn::Error::new_spanned(expr, "expected integer literal for base_delay_ms").to_compile_error().into(),
                    },
                    "max_delay_ms" => match expr {
                        Expr::Lit(syn::ExprLit { lit: Lit::Int(litint), .. }) => max_delay_ms = Some(litint.base10_parse::<u64>().unwrap()),
                        _ => return syn::Error::new_spanned(expr, "expected integer literal for max_delay_ms").to_compile_error().into(),
                    },
                    "backoff_factor" => match expr {
                        Expr::Lit(syn::ExprLit { lit: Lit::Float(litf), .. }) => backoff_factor = Some(litf.base10_parse::<f64>().unwrap()),
                        Expr::Lit(syn::ExprLit { lit: Lit::Int(liti), .. }) => backoff_factor = Some(liti.base10_parse::<f64>().unwrap()),
                        _ => return syn::Error::new_spanned(expr, "expected numeric literal for backoff_factor").to_compile_error().into(),
                    },
                    "jitter" => match expr {
                        Expr::Lit(syn::ExprLit { lit: Lit::Bool(litb), .. }) => jitter_opt = Some(litb.value),
                        _ => return syn::Error::new_spanned(expr, "expected boolean literal for jitter").to_compile_error().into(),
                    },
                    "rng_seed" => match expr {
                        Expr::Lit(syn::ExprLit { lit: Lit::Int(litint), .. }) => rng_seed = Some(litint.base10_parse::<u64>().unwrap()),
                        _ => return syn::Error::new_spanned(expr, "expected integer literal for rng_seed").to_compile_error().into(),
                    },
                    "predicate" => {
                        // Accept either a bare path (Expr::Path) or a string literal with the path
                        match expr {
                            Expr::Path(_) => {
                                // Use as-is
                                // We'll store this expression directly in predicate_expr below
                                predicate_expr = Some(expr);
                            }
                            Expr::Lit(syn::ExprLit { lit: Lit::Str(lits), .. }) => {
                                // Parse string into a path
                                let s = lits.value();
                                match s.parse::<syn::Path>() {
                                    Ok(p) => predicate_expr = Some(Expr::Path(syn::ExprPath { attrs: Vec::new(), qself: None, path: p })),
                                    Err(_) => return syn::Error::new_spanned(lits, "invalid path in string").to_compile_error().into(),
                                }
                            }
                            _ => return syn::Error::new_spanned(expr, "expected path or string literal for predicate").to_compile_error().into(),
                        }
                    }
                    other => return syn::Error::new_spanned(ident, format!("unknown option `{}`", other)).to_compile_error().into(),
                }
            }
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

    // Build policy initializer fields
    let mut fields = Vec::new();
    fields.push(quote! { attempts: #attempts });
    if let Some(ms) = base_delay_ms {
        fields.push(quote! { base_delay: ::std::time::Duration::from_millis(#ms) });
    }
    if let Some(ms) = max_delay_ms {
        fields.push(quote! { max_delay: ::std::time::Duration::from_millis(#ms) });
    }
    if let Some(f) = backoff_factor {
        fields.push(quote! { backoff_factor: #f });
    }
    if let Some(b) = jitter_opt {
        fields.push(quote! { jitter: #b });
    }
    if let Some(seed) = rng_seed {
        fields.push(quote! { rng_seed: Some(#seed) });
    }

    // predicate expression to use as the retry predicate; defaults to `|_| true`
    let predicate_tokens = if let Some(pred) = predicate_expr {
        quote! { #pred }
    } else {
        quote! { |_| true }
    };

    let expanded = quote! {
        #(#attrs)*
        #vis #sig {
            let policy = ::asyn_retry_policy::RetryPolicy { #(#fields),*, ..Default::default() };
            policy.retry(|| {
                #(#clones)*
                async move #block
            }, #predicate_tokens).await
        }
    };

    expanded.into()
}
