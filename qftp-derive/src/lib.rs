use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{self, Ident};

struct TokenStorage {
    pub recv: Vec<TokenStream2>,
    pub to_bytes: Vec<TokenStream2>,
}

impl TokenStorage {
    fn new() -> Self {
        TokenStorage {
            recv: vec![],
            to_bytes: vec![],
        }
    }
}

#[proc_macro_derive(Message)]
pub fn message_macro_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_message_macro(&ast)
}

const NUMERIC_TYPES: [&str; 12] = [
    "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "f32", "f64",
];

// TODO: support simple enums
fn gen_append(fields: &Punctuated<syn::Field, syn::token::Comma>) -> TokenStorage {
    let mut token_storage = TokenStorage::new();
    // loop over every field in the struct
    let mut previous_field_ident = None;
    for field in fields {
        // field ident is the name of the field
        let field_ident = field.ident.as_ref().unwrap();

        // path_segment here is the last segment of the path
        // i.e. std::fs::Path => Path
        // u8 => u8
        let ty = if let syn::Type::Path(syn::TypePath { ref path, .. }) = field.ty {
            path.segments.last().unwrap()
        } else {
            unimplemented!("Only TypePath supported for now")
        };

        // If the type is numeric (u8, u16, ...), directly add TokenStream
        if NUMERIC_TYPES.contains(&ty.ident.to_string().as_str()) {
            gen_for_numeric(field_ident, &ty.ident, &mut token_storage);
            previous_field_ident = Some(field_ident);
        } else if ty.ident == "Vec" {
            // Else we have to loop over the vector
            if let Some(previous_field_ident) = previous_field_ident {
                gen_for_vec(field, ty, previous_field_ident, &mut token_storage);
            } else {
                panic!("the field directly before a collection has to be of numeric type");
            }
            previous_field_ident = None;
        } else if ty.ident == "String" {
            if let Some(previous_field_ident) = previous_field_ident {
                gen_for_str_types(field_ident, previous_field_ident, &mut token_storage);
            } else {
                panic!("the field directly before a String has to be of numeric type");
            }
            previous_field_ident = None;
        } else {
            panic!("Unsupported type `{}` on field `{}`", ty.ident, field_ident);
        }
    }

    token_storage
}

fn gen_for_str_types(
    field_ident: &Ident,
    previous_field_ident: &Ident,
    token_storage: &mut TokenStorage,
) {
    let token_to_bytes = quote!(v.extend_from_slice(&self.#field_ident.as_bytes()));
    let token_recv = quote! {
        let mut buf: Vec<u8> = vec![0; #previous_field_ident.try_into().unwrap()];  
        s.read_exact(&mut buf[..]).await?;
        let #field_ident = String::from_utf8(buf).unwrap();
    };

    token_storage.to_bytes.push(token_to_bytes);
    token_storage.recv.push(token_recv);
}

fn gen_for_numeric(field_ident: &Ident, type_ident: &Ident, token_storage: &mut TokenStorage) {
    // Note: we assume the final output Vector is called v
    let tokens_to_bytes = quote!(v.extend_from_slice(&self.#field_ident.to_be_bytes()));
    let function_name = Ident::new(format!("read_{}", type_ident).as_str(), Span::call_site());
    let tokens_recv = quote!(let #field_ident = s.#function_name().await?);
    token_storage.to_bytes.push(tokens_to_bytes);
    token_storage.recv.push(tokens_recv);
}

fn gen_for_vec(
    field: &syn::Field,
    ty: &syn::PathSegment,
    previous_field_ident: &Ident,
    token_storage: &mut TokenStorage,
) {
    // get the generic type
    // generics here is the list of arguments in AngleBrackets
    // Vec<u32> => [u32]
    // HashMap<u32, String> => [u32, String]
    let generic = if let syn::PathSegment {
        arguments:
            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { ref args, .. }),
        ..
    } = ty
    {
        // we only support Vectors here so the generic argument should just be one
        assert_eq!(args.len(), 1, "Generic arg to Vec has to be 1");
        args.first().unwrap()
    } else {
        unimplemented!("Generic arg to Vec has to be AngleBracketedGenericArguments")
    };

    // Type of the generic
    let ty_generic = if let syn::GenericArgument::Type(syn::Type::Path(syn::TypePath {
        path: syn::Path { ref segments, .. },
        ..
    })) = generic
    {
        segments.last().unwrap()
    } else {
        unimplemented!("Generic arg to Vec has to be TypePath")
    };

    // name of the generic type
    let ty_generic_ident = &ty_generic.ident;

    // only Vectors with numeric generic are allowed
    if !NUMERIC_TYPES.contains(&ty_generic_ident.to_string().as_str()) {
        unimplemented!("arg to Vec has to be numeric")
    }

    // name of the field
    let field_ident = field.ident.as_ref().unwrap();

    let token_stream = quote! {
        for el in self.#field_ident {
            v.extend_from_slice(&el.to_be_bytes());
        }
    };

    let function_name = Ident::new(
        format!("read_{}", ty_generic_ident).as_str(),
        Span::call_site(),
    );
    let token_stream_recv = quote! {
        let mut #field_ident: Vec<#ty_generic_ident> = Vec::new();

        for _ in 0..#previous_field_ident {
            #field_ident.push(s.#function_name().await?);
        }

    };
    token_storage.recv.push(token_stream_recv);
    token_storage.to_bytes.push(token_stream);
}

fn impl_message_macro(ast: &syn::DeriveInput) -> TokenStream {
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
    let field_names = fields.iter().map(|field| field.ident.as_ref().unwrap());
    let ts_recv = &ts.recv;
    let ts_to_bytes = &ts.to_bytes;

    let struct_name = &ast.ident;

    let gen = quote! {
        #[async_trait::async_trait]
        impl Message for #struct_name {
            async fn recv<T>(s: &mut T) -> Result<Self, Error>
            where
                Self: Sized,
                T: Sync + Send + Unpin + tokio::io::AsyncRead,
            {
                use tokio::io::AsyncReadExt;
                #(#ts_recv);*;

                Ok(#struct_name {
                    #(#field_names),*
                })
            }

            fn to_bytes(self) -> Vec<u8> {
                let mut v = Vec::new();

                #(#ts_to_bytes);*;

                v
            }
        }
    };

    gen.into()
}
