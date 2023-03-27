use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Expr, ExprLit, Fields, ItemStruct, Lit, LitInt, parse_macro_input, Type};
use syn::punctuated::Punctuated;


fn u64_to_some_expr(value: u64) -> Option<Expr> {
    Some(Expr::Lit(ExprLit {
        attrs: Vec::new(),
        lit: Lit::Int(LitInt::new(&value.to_string(), Span::call_site())),
    }))
}

fn minimum_length_for_type(ty: &Type) -> Option<Expr> {
    if let Type::Path(pth) = ty {
        if pth.path.segments.len() == 1 {
            let seg = pth.path.segments.first().unwrap();
            let ident = seg.ident.to_string();
            if seg.arguments.is_none() {
                match ident.as_str() {
                    "u8"|"i8" => return u64_to_some_expr(1),
                    "u16"|"i16" => return u64_to_some_expr(2),
                    "u32"|"i32" => return u64_to_some_expr(4),
                    "u64"|"i64" => return u64_to_some_expr(8),
                    "u128"|"i128" => return u64_to_some_expr(16),
                    _ => {},
                }
            }
        }
    }
    return None;
}


#[proc_macro_attribute]
pub fn minimum_length(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as ItemStruct);

    let mut biased = false;
    let attr_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("biased") {
            biased = true;
            Ok(())
        } else {
            Err(meta.error("unsupported minimum_length parameter"))
        }
    });
    parse_macro_input!(attr with attr_parser);

    let no_fields = Punctuated::default();
    let struct_fields = match &ast.fields {
        Fields::Unit => &no_fields,
        Fields::Named(nf) => &nf.named,
        Fields::Unnamed(uf) => &uf.unnamed,
    };

    let mut size_pieces = Vec::new();
    for field in struct_fields {
        if let Some(ml) = minimum_length_for_type(&field.ty) {
            size_pieces.push(ml);
        }
    }

    let min_length_base_expr = if biased {
        quote! { Self::min_len_bias() }
    } else {
        quote! { 0 }
    };

    let struct_name = &ast.ident;
    let implementation = quote! {
        #ast
        impl #struct_name {
            pub const fn min_len() -> u64 {
                #min_length_base_expr #( + #size_pieces )*
            }
        }
    };
    implementation.into()
}
