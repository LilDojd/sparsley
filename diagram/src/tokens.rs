//! Token trees detached from `proc_macro`, so parsing can be unit tested.

/// A token. `span` indexes the spans kept beside the trees, so errors can
/// point at the offending token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Tree {
    Ident {
        name: String,
        span: usize,
    },
    Punct {
        ch: char,
        span: usize,
    },
    Literal {
        text: String,
        span: usize,
    },
    Group {
        delimiter: Delimiter,
        trees: Vec<Tree>,
        span: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delimiter {
    Parenthesis,
    Bracket,
    Brace,
}

impl Tree {
    pub(crate) fn span(&self) -> usize {
        match *self {
            Self::Ident { span, .. }
            | Self::Punct { span, .. }
            | Self::Literal { span, .. }
            | Self::Group { span, .. } => span,
        }
    }
}

/// Converts a macro input, returning the trees and the span of each id.
/// Invisible groups are flattened.
pub(crate) fn read(stream: proc_macro::TokenStream) -> (Vec<Tree>, Vec<proc_macro::Span>) {
    let mut spans = Vec::new();
    let trees = convert(stream, &mut spans);
    (trees, spans)
}

fn convert(stream: proc_macro::TokenStream, spans: &mut Vec<proc_macro::Span>) -> Vec<Tree> {
    use proc_macro::{Delimiter as D, TokenTree as T};

    let mut trees = Vec::new();
    for token in stream {
        let span = spans.len();
        spans.push(token.span());
        trees.push(match token {
            T::Ident(ident) => Tree::Ident {
                name: ident.to_string(),
                span,
            },
            T::Punct(punct) => Tree::Punct {
                ch: punct.as_char(),
                span,
            },
            T::Literal(literal) => Tree::Literal {
                text: literal.to_string(),
                span,
            },
            T::Group(group) => {
                let delimiter = match group.delimiter() {
                    D::Parenthesis => Delimiter::Parenthesis,
                    D::Bracket => Delimiter::Bracket,
                    D::Brace => Delimiter::Brace,
                    D::None => {
                        trees.extend(convert(group.stream(), spans));
                        continue;
                    }
                };
                let trees = convert(group.stream(), spans);
                Tree::Group {
                    delimiter,
                    trees,
                    span,
                }
            }
        });
    }
    trees
}

/// Lexes the subset of Rust the macros accept, returning the trees and the
/// text of each span id.
#[cfg(test)]
pub(crate) fn lex(source: &str) -> (Vec<Tree>, Vec<String>) {
    let mut texts = Vec::new();
    let mut chars = source.chars().peekable();
    let trees = lex_until(&mut chars, None, &mut texts);
    (trees, texts)
}

#[cfg(test)]
fn lex_until(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    close: Option<char>,
    texts: &mut Vec<String>,
) -> Vec<Tree> {
    let mut trees = Vec::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if Some(c) == close {
            chars.next();
            return trees;
        }
        let span = texts.len();
        let take = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>, f: fn(char) -> bool| {
            let mut text = String::new();
            while let Some(&c) = chars.peek().filter(|&&c| f(c)) {
                text.push(c);
                chars.next();
            }
            text
        };
        let tree = match c {
            '(' | '[' | '{' => {
                chars.next();
                texts.push(c.to_string());
                let (delimiter, close) = match c {
                    '(' => (Delimiter::Parenthesis, ')'),
                    '[' => (Delimiter::Bracket, ']'),
                    _ => (Delimiter::Brace, '}'),
                };
                let trees = lex_until(chars, Some(close), texts);
                Tree::Group {
                    delimiter,
                    trees,
                    span,
                }
            }
            '\'' => {
                let mut text = String::from(chars.next().unwrap());
                while let Some(c) = chars.next() {
                    text.push(c);
                    if c == '\\' {
                        text.extend(chars.next());
                    } else if c == '\'' {
                        break;
                    }
                }
                texts.push(text.clone());
                Tree::Literal { text, span }
            }
            c if c.is_ascii_digit() => {
                let text = take(chars, |c| c.is_ascii_alphanumeric() || c == '_');
                texts.push(text.clone());
                Tree::Literal { text, span }
            }
            c if c.is_alphabetic() || c == '_' => {
                let name = take(chars, |c| c.is_alphanumeric() || c == '_');
                texts.push(name.clone());
                Tree::Ident { name, span }
            }
            ch => {
                chars.next();
                texts.push(ch.to_string());
                Tree::Punct { ch, span }
            }
        };
        trees.push(tree);
    }
    trees
}
