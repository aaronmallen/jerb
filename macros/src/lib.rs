use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, GenericArgument, ItemFn, PathArguments, Type, parse_macro_input};

#[proc_macro_attribute]
pub fn job(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let item = parse_macro_input!(item as ItemFn);
  let name = &item.sig.ident;
  let vis = &item.vis;
  let args = extractor(&item, "Json").map_or_else(|| quote!(()), |args| quote!(#args));
  let (state, call) = extractor(&item, "State").map_or_else(
    || (quote!(None), quote!(|run, _| ::jerb::Handler::call(#name, run, ()))),
    |state| {
      (
        quote!(Some(::core::any::TypeId::of::<#state>)),
        quote!(|run, state| ::jerb::__call::<#state, _, _>(#name, run, state)),
      )
    },
  );

  quote! {
    #item

    #vis mod #name {
      #[allow(unused_imports)]
      use super::*;

      pub async fn enqueue(
        client: &impl ::jerb::Enqueue,
        args: #args,
      ) -> ::core::result::Result<i64, ::std::boxed::Box<dyn ::std::error::Error + Send + Sync>> {
        ::jerb::Enqueue::enqueue(client, stringify!(#name), ::jerb::__serde_json::to_value(args)?).await
      }
    }

    ::jerb::__inventory::submit! {
      ::jerb::Route::new(stringify!(#name), #state, #call)
    }
  }
  .into()
}

fn extractor<'a>(item: &'a ItemFn, extractor: &str) -> Option<&'a Type> {
  item.sig.inputs.iter().find_map(|input| {
    let FnArg::Typed(input) = input else { return None };
    let Type::Path(path) = &*input.ty else { return None };
    let segment = path.path.segments.last().filter(|segment| segment.ident == extractor)?;
    let PathArguments::AngleBracketed(generics) = &segment.arguments else {
      return None;
    };
    generics.args.iter().find_map(|arg| match arg {
      GenericArgument::Type(ty) => Some(ty),
      _ => None,
    })
  })
}
