//! Memory diagrams of `sparsley` operations, drawn at compile time for
//! rustdoc.
//!
//! [`diagram!`] runs Rust-like statements on a reference model of
//! `SparseMap` or `SparseSet` and expands to a fenced `text` block:
//!
//! ```
//! #[doc = sparsley_diagram::diagram! {
//!     let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c')]);
//!     map.remove(3);
//! }]
//! pub struct Documented;
//! ```
//!
//! [`state!`] takes the same input and expands to the final model state, so
//! tests can check the model against the real collections.

mod grid;
mod link;
mod model;
mod notes;
mod parse;
mod render;
mod snapshot;
mod table;
mod tokens;

#[cfg(test)]
mod tests;

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

use model::Run;
use parse::{Error, Program};

/// Expands to a string literal holding a fenced `text` diagram of each call.
///
/// ```
/// const DIAGRAM: &str = sparsley_diagram::diagram! {
///     let mut set = SparseSet::from([3, 7, 1]);
///     set.insert(9);
/// };
/// assert!(DIAGRAM.starts_with("```text\nset.insert(9) -> true\n"));
/// ```
///
/// The input is one constructor statement, `SparseMap::new()`,
/// `SparseMap::with_capacity(n)` or `SparseMap::from([(key, value), ...])`
/// (and the same for `SparseSet` with plain keys), followed by method calls.
/// Keys are `usize` literals; values are `char` or integer literals.
/// Unsupported input is a compile error at the offending token.
#[proc_macro]
pub fn diagram(input: TokenStream) -> TokenStream {
    expand(input, |_, run| {
        Ok(TokenTree::from(Literal::string(&run.diagram())).into())
    })
}

/// Expands to the final model state as a tuple:
/// `(slots, keys, values, len, capacity, key_capacity)`.
///
/// `slots` is a `&[Option<usize>]` of dense indices for every key below
/// `key_capacity`, `keys` a `&[usize]`, and `values` a `&[char]`, `&[i64]`
/// or, for a set, an empty `&[()]`.
///
/// ```
/// let (slots, keys, values, len, capacity, key_capacity) = sparsley_diagram::state! {
///     let mut map = SparseMap::from([(3, 'a'), (7, 'b')]);
///     map.remove(3);
/// };
/// assert_eq!((keys, values, len, capacity, key_capacity), (&[7][..], &['b'][..], 1, 2, 64));
/// assert_eq!(slots[7], Some(0));
/// ```
#[proc_macro]
pub fn state(input: TokenStream) -> TokenStream {
    expand(input, |program, run| {
        run.state(program.value_type).parse().map_err(|_| Error {
            message: "sparsley-diagram generated invalid state source".to_owned(),
            span: None,
        })
    })
}

/// Parses and runs `input`, then builds the expansion, or a compile error at
/// the offending token.
fn expand(
    input: TokenStream,
    build: fn(&Program, &Run<'_>) -> Result<TokenStream, Error>,
) -> TokenStream {
    let (trees, spans) = tokens::read(input);
    let expansion =
        parse::program(&trees).and_then(|program| build(&program, &Run::new(&program)?));
    expansion.unwrap_or_else(|error| compile_error(&error, &spans))
}

/// Builds `compile_error!("message")` spanned at the offending token.
fn compile_error(error: &Error, spans: &[Span]) -> TokenStream {
    let span = error
        .span
        .and_then(|span| spans.get(span).copied())
        .unwrap_or_else(Span::call_site);
    let mut message = Literal::string(&error.message);
    message.set_span(span);
    let mut bang = Punct::new('!', Spacing::Alone);
    bang.set_span(span);
    let mut arguments = Group::new(Delimiter::Parenthesis, TokenTree::from(message).into());
    arguments.set_span(span);
    let tokens: [TokenTree; 3] = [
        Ident::new("compile_error", span).into(),
        bang.into(),
        arguments.into(),
    ];
    tokens.into_iter().collect()
}
