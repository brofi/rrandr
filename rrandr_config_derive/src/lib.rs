use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, AngleBracketedGenericArguments, Attribute, Data, DataStruct, DeriveInput,
    Expr, Field, Fields, GenericArgument, Lit, Meta, PathArguments, PathSegment, Type,
};

#[proc_macro_derive(MarkdownTable, attributes(table))]
pub fn derive_markdown_table(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_markdown_table(ast)
}

fn impl_markdown_table(ast: DeriveInput) -> TokenStream {
    let name = ast.ident;
    let desc = get_doc(&ast.attrs) + "\n\n";
    let header = ["Attribute", "Type", "Default", "Description"];
    let align = "|-".repeat(header.len()) + "|\n";
    let header = String::from("| ") + &header.join(" | ") + " |\n";
    match ast.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields), .. }) => {
            let rows = fields
                .named
                .iter()
                .filter(|&f| !is_skip(f) && !is_table(f))
                .map(|f| {
                    let attr_name = &f.ident;
                    let ty = ty_to_toml(&f.ty);
                    let desc = get_doc(&f.attrs).trim().to_owned();
                    quote! {
                        "| `" + stringify!(#attr_name)
                        + "` | `" + #ty
                        + "` | `" + &#name::default().#attr_name.to_string()
                        + "` | " + #desc + " |\n"
                    }
                })
                .collect::<Vec<_>>();

            let tables = fields.named.iter().filter(|&f| !is_skip(f) && is_table(f)).map(|f| {
                let attr_name = &f.ident;
                let ty = &f.ty;
                quote! {
                    "\n" + #ty::to_markdown_table(&(String::from(key)
                    + if key.is_empty() {""} else {"."}
                    + stringify!(#attr_name)), lvl).as_str()
                }
            });

            let body = if rows.len() > 0 {
                quote! { "#".repeat(lvl.into()) + " `[" + key + "]`" + #desc + #header + #align #(+ #rows)* }
            } else {
                quote! { String::new() }
            };

            quote! {
                impl MarkdownTable for #name {
                    fn to_markdown_table(key: &str, lvl: u8) -> String { #body #(+ #tables)* }
                }
            }
            .into()
        }
        _ => panic!("Only structs with named fields supported"),
    }
}

fn is_skip(field: &Field) -> bool {
    let mut skip = false;
    for attr in &field.attrs {
        if attr.path().is_ident("serde") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    skip = true;
                }
                Ok(())
            });
            break;
        }
    }
    skip
}

fn is_table(field: &Field) -> bool {
    for attr in &field.attrs {
        if attr.path().is_ident("table") {
            return true;
        }
    }
    false
}

fn get_doc(attrs: &[Attribute]) -> String {
    let mut desc = String::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(meta) = &attr.meta {
                if let Expr::Lit(expr) = &meta.value {
                    if let Lit::Str(lit) = &expr.lit {
                        desc.push_str(&lit.value());
                    }
                }
            }
        }
    }
    desc
}

fn ty_to_toml(ty: &Type) -> String {
    if let Type::Path(ty) = ty {
        let ty = ty.path.segments.last().expect("should have path segment");
        if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
            &ty.arguments
        {
            if let Some(GenericArgument::Type(gty)) = args.first() {
                if let Type::Path(gty) = gty {
                    let gty = gty.path.segments.last().expect("should have path segment");
                    let ident = ty.ident.to_string();
                    if ident == "Auto" {
                        return ty_path_seg_to_toml(gty) + " or \"auto\"";
                    }
                }
            }
        }
        return ty_path_seg_to_toml(ty);
    }
    String::from("unknown")
}

fn ty_path_seg_to_toml(seg: &PathSegment) -> String {
    let ident = seg.ident.to_string();
    String::from(match ident.as_str() {
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => "Integer",
        "f32" | "f64" => "Float",
        "bool" => "Boolean",
        _ => &ident,
    })
}
