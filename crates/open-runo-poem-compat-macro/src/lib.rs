//! poemの`#[handler]`属性マクロと同じ書き味を提供する。
//!
//! **正直な開示・スコープ**: poem本体の`#[handler]`は、関数引数の型
//! (`Data<&T>`・`Path<T>`・`Json<T>`等)を見て自動的に`FromRequest`
//! 抽出コードを生成する高度なマクロだが、本マクロはそこまでの型駆動
//! 抽出は行わない。対象にできるのは、シグネチャが
//! `async fn name(req: &open_runo_poem_compat::Request,
//! params: open_runo_poem_compat::Params) -> open_runo_poem_compat::Response`
//! (または`Request`所有版)の関数のみ——生成されるのは、この関数を
//! `open_runo_poem_compat::Handler`(`Arc<dyn Fn(Request, Params) ->
//! BoxFuture<Response> + Send + Sync>`)へ包む同名の付随関数であり、
//! `get(name())`のようにpoemの`get(handler_name)`と同じ書き味で
//! ルーティングへ渡せるようにする、というのが本マクロの範囲。
//! 型駆動の`Data`/`Path`/`Json`自動抽出は今後の増分。

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);
    let vis = input.vis.clone();
    let name = input.sig.ident.clone();
    let inner_name = syn::Ident::new(&format!("__{}_impl", name), name.span());
    input.sig.ident = inner_name.clone();
    // 内部実装関数は外からは見えなくてよい(公開APIは下の`#name()`のみ)。
    input.vis = syn::Visibility::Inherited;

    let expanded = quote! {
        #input

        #vis fn #name() -> ::open_runo_poem_compat::Handler {
            ::open_runo_poem_compat::handler_fn(#inner_name)
        }
    };

    TokenStream::from(expanded)
}
