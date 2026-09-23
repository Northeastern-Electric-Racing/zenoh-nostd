use proc_macro2::TokenStream;
use syn::DeriveInput;

pub mod info;
pub mod rx;
pub mod tx;

pub fn derive_zlink(input: &DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let (_, ty_generics, _) = input.generics.split_for_impl();

    // `ZLink` is parameterized by the lifetime of the underlying resource
    // (e.g. a socket's network stack handle), fixed per-impl rather than
    // re-elided on every `split()` call. If the deriving type already
    // declares a lifetime of its own (by convention the last one, matching
    // `Foo<'buf, 'net>`), reuse it directly. Otherwise (no inherent
    // "stack"-like lifetime, e.g. owned/std sockets) synthesize a fresh one
    // that's universally quantified.
    let mut impl_generics_input = input.generics.clone();
    let zlink_lifetime = match input.generics.lifetimes().last() {
        Some(lt) => lt.lifetime.clone(),
        None => {
            let lt: syn::Lifetime = syn::parse_quote!('__zlink);
            impl_generics_input.params.insert(0, syn::parse_quote!(#lt));
            lt
        }
    };
    let (impl_generics, _, where_clause) = impl_generics_input.split_for_impl();
    let (tx_type, rx_type) = extract_zlink_types(input)?;
    let variants = match &input.data {
        syn::Data::Enum(data_enum) => &data_enum.variants,
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "ZLInk can only be derived for enums",
            ));
        }
    }
    .iter()
    .map(|variant| &variant.ident);

    let variants_split = variants.clone().map(|ident| {
        quote::quote! {
            Self:: #ident (link) => {
                let (tx, rx) = zenoh_nostd::platform::ZLink::split(link);
                (Self::Tx:: #ident (tx), Self::Rx:: #ident (rx))
            },
        }
    });

    Ok(quote::quote! {
        impl #impl_generics zenoh_nostd::platform::ZLink<#zlink_lifetime> for #ident #ty_generics #where_clause {
            type Tx<'link> = #tx_type where Self: 'link;
            type Rx<'link> = #rx_type where Self: 'link;

            fn split(&mut self) -> (Self::Tx<'_>, Self::Rx<'_>) {
                match self {
                    #(#variants_split)*
                }
            }
        }
    })
}

fn extract_zlink_types(input: &DeriveInput) -> syn::Result<(syn::Type, syn::Type)> {
    for attr in &input.attrs {
        if !attr.path().is_ident("zenoh") {
            continue;
        }

        let result: Result<(syn::Type, syn::Type), syn::Error> =
            attr.parse_args_with(|input: syn::parse::ParseStream| {
                let name: syn::Ident = input.parse()?;
                if name != "ZLink" {
                    return Err(syn::Error::new(name.span(), "Expected 'ZLink'"));
                }

                input.parse::<syn::Token![=]>()?;

                let content;
                syn::parenthesized!(content in input);

                let tx: syn::Type = content.parse()?;
                content.parse::<syn::Token![,]>()?;
                let rx: syn::Type = content.parse()?;

                Ok((tx, rx))
            });

        if let Ok(types) = result {
            return Ok(types);
        }
    }

    Err(syn::Error::new_spanned(
        input,
        "Missing #[zenoh(ZLink = (TxType, RxType))] attribute",
    ))
}
