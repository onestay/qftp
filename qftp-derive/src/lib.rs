use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{self, Ident, PathSegment};

#[proc_macro_derive(Message)]
pub fn message_macro_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_message_macro(&ast)
}

const NUMERIC_TYPES: [&str; 12] = [
    "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "f32", "f64",
];

fn gen_append(fields: &Punctuated<syn::Field, syn::token::Comma>) -> Vec<TokenStream2> {
    let mut token_stream = Vec::new();

    // loop over every field in the struct
    for field in fields {
        // field ident is the name of the field
        let field_ident = field.ident.as_ref().unwrap();

        // path_segment here is the last segment of the path
        // i.e. std::fs::Path => Path
        // u8 => u8
        let path_segment = if let syn::Type::Path(syn::TypePath { ref path, .. }) = field.ty {
            path.segments.last().unwrap()
        } else {
            unimplemented!("Only TypePath supported for now")
        };

        // If the type is numeric (u8, u16, ...), directly add 
        if NUMERIC_TYPES.contains(&path_segment.ident.to_string().as_str()) {
            token_stream.push(gen_for_numeric(field.ty.span(), field_ident));
        } else if path_segment.ident == "Vec" {
            token_stream.push(gen_for_vec(field, path_segment));
        }
    }

    token_stream
}

fn gen_for_numeric(span: proc_macro2::Span, field_ident: &Ident) -> TokenStream2 {
    let tokens = quote_spanned!(span=> v.extend_from_slice(&self.#field_ident.to_be_bytes()));

    tokens
}

fn gen_for_vec(field: &syn::Field, path_segement: &syn::PathSegment) -> TokenStream2 {
    let generics = if let syn::PathSegment {
        arguments:
            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { ref args, .. }),
        ..
    } = path_segement
    {
        args
    } else {
        unimplemented!("Generic arg to Vec has to be AngleBracketedGenericArguments")
    };

    if generics.len() != 1 {
        unimplemented!("Generic arg to Vec has to be 1")
    }

    let generics = generics.into_iter().next().unwrap();
    let path = if let syn::GenericArgument::Type(syn::Type::Path(syn::TypePath{path: syn::Path{ref segments, ..}, ..})) = generics {
        segments.last().unwrap()
    } else {
        unimplemented!("Generic arg to Vec has to be TypePath")
    };
    
    let ident = &path.ident;

    if !NUMERIC_TYPES.contains(&ident.to_string().as_str()) {
        unimplemented!("arg to Vec has to be numeric")
    }

    let vec_field_ident = field.ident.as_ref().unwrap();
    let outer = quote! {
        for el in self.#vec_field_ident {
            v.extend_from_slice(&el.to_be_bytes());
        }
    };

    outer
}

fn impl_message_macro(ast: &syn::DeriveInput) -> TokenStream {
    eprintln!("{:#?}", ast);
    let fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
        ..
    }) = ast.data
    {
        named
    } else {
        unimplemented!("Only named structs are supported")
    };

    let ts = gen_append(fields);

    eprintln!("{:?}", ts);

    //eprintln!("{:#?}", ast);
    let name = &ast.ident;

    let gen = quote! {
        #[async_trait::async_trait]
        impl Message for #name {
            async fn recv<T>(control_stream: &mut T) -> Result<Self, Error>
            where
                Self: Sized,
                T: Sync + Send + Unpin + tokio::io::AsyncRead,
            {
                use tokio::io::AsyncReadExt;
                todo!()
            }

            fn to_bytes(self) -> Vec<u8> {
                let mut v = Vec::new();

                #(#ts);*;

                v
            }
        }
    };

    gen.into()
}
