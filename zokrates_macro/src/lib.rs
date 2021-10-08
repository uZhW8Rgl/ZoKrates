extern crate proc_macro;

use quote::quote;
use syn::*;

#[proc_macro_attribute]
pub fn stopwatch(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(mut fun) = syn::parse::<ItemFn>(item.clone()) {
            fun.block.stmts = rewrite_function(fun.sig.ident.to_string(), &mut fun.block.stmts);
            return quote!(#fun).into();
        }

        if let Ok(mut fun) = syn::parse::<TraitItemMethod>(item.clone()) {
            if let Some(block) = fun.default.as_mut() {
                block.stmts = rewrite_function(fun.sig.ident.to_string(), &mut block.stmts);
                return quote!(#fun).into();
            }
        }

        if let Ok(mut fun) = syn::parse::<ImplItemMethod>(item) {
            fun.block.stmts = rewrite_function(fun.sig.ident.to_string(), &mut fun.block.stmts);
            return quote!(#fun).into();
        }

        panic!("`zokrates_macro::stopwatch` should be used on functions")
    }

    #[cfg(target_arch = "wasm32")]
    item
}

fn rewrite_function(name: String, statements: &mut Vec<Stmt>) -> Vec<Stmt> {
    let mut block: Block = parse_quote! {{
        pub struct Stopwatch(std::time::Instant);

        impl Drop for Stopwatch {
            fn drop(&mut self) {
                log::debug!("Stopwatch: function `{}` was running for {}ms", #name, self.0.elapsed().as_millis());
            }
        }

        let _stopwatch = Stopwatch(std::time::Instant::now());
    }};

    block.stmts.append(statements);
    block.stmts
}
