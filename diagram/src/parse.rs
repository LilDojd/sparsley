//! Parses the macro input: one setup statement, then method calls.
//!
//! ```text
//! let mut map = SparseMap::from([(3, 'a'), (7, 'b')]);
//! map.remove(3);
//! ```

use std::fmt;

use crate::snapshot::Kind;
use crate::tokens::{Delimiter, Tree};

/// A parse or evaluation error, pointing at a token if `span` is set.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Error {
    pub(crate) message: String,
    pub(crate) span: Option<usize>,
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// A parsed macro input.
#[derive(Debug)]
pub(crate) struct Program {
    pub(crate) kind: Kind,
    /// The Rust type of the values: `char`, `i64`, or `()` for a set.
    pub(crate) value_type: &'static str,
    pub(crate) init: Init,
    pub(crate) calls: Vec<Call>,
}

/// How the collection is constructed.
#[derive(Debug)]
pub(crate) enum Init {
    New,
    WithCapacity(usize),
    /// `from([...])`; set entries have no value.
    From(Vec<(usize, Option<Value>)>),
}

/// A drawn method call.
#[derive(Debug)]
pub(crate) struct Call {
    /// The call as written, normalized, such as `map.remove(3)`.
    pub(crate) text: String,
    pub(crate) op: Op,
    pub(crate) span: usize,
}

/// A method call, by behavior. Methods that behave alike share a variant.
#[derive(Debug)]
pub(crate) enum Op {
    Insert {
        key: usize,
        value: Option<Value>,
    },
    /// `get` or `get_mut`.
    Get(usize),
    GetKeyValue(usize),
    /// `entry(key).or_insert(value)`.
    EntryOrInsert {
        key: usize,
        value: Value,
    },
    /// `contains_key` or `contains`.
    Contains(usize),
    GetIndexOf(usize),
    GetIndex(usize),
    Remove(usize),
    RemoveEntry(usize),
    SwapRemoveIndex(usize),
    SwapIndices(usize, usize),
    Clear,
    Retain(Predicate),
    Reserve(usize),
    ReserveKeys(usize),
    ShrinkToFit,
    /// `sort_unstable_keys` or `sort_unstable`.
    Sort,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Value {
    Char(char),
    Int(i64),
}

/// A `retain` closure: `|key, value| <subject> [% modulo] <cmp> <literal>`.
#[derive(Debug)]
pub(crate) struct Predicate {
    pub(crate) text: String,
    pub(crate) subject: Subject,
    pub(crate) modulo: Option<i64>,
    pub(crate) cmp: Cmp,
    pub(crate) literal: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Subject {
    Key,
    Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Error {
    fn at(span: Option<usize>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl Value {
    pub(crate) fn kind_name(self) -> &'static str {
        match self {
            Self::Char(_) => "char",
            Self::Int(_) => "integer",
        }
    }
}

/// Displays a value as drawn in a cell.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(c) => write!(f, "{c}"),
            Self::Int(n) => write!(f, "{n}"),
        }
    }
}

/// Displays a value as Rust source and `Debug` print it.
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(c) => write!(f, "{c:?}"),
            Self::Int(n) => write!(f, "{n}"),
        }
    }
}

impl Cmp {
    fn symbol(self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }

    pub(crate) fn holds(self, left: Value, right: Value) -> bool {
        match self {
            Self::Eq => left == right,
            Self::Ne => left != right,
            Self::Lt => left < right,
            Self::Le => left <= right,
            Self::Gt => left > right,
            Self::Ge => left >= right,
        }
    }
}

/// Parses a whole macro input.
pub(crate) fn program(trees: &[Tree]) -> Result<Program> {
    let mut input = Cursor::new(trees, None);
    let mut values = ValueType::default();
    let (name, kind, init) = setup(&mut input, &mut values)?;
    let mut calls = Vec::new();
    while !input.is_empty() {
        calls.push(call(&mut input, &name, kind, &mut values)?);
    }
    let value_type = match (kind, values.0) {
        (Kind::Set, _) => "()",
        (Kind::Map, Some(Value::Int(_))) => "i64",
        (Kind::Map, _) => "char",
    };
    Ok(Program {
        kind,
        value_type,
        init,
        calls,
    })
}

/// `let mut <name> = <Type>::<constructor>;`
fn setup(input: &mut Cursor<'_>, values: &mut ValueType) -> Result<(String, Kind, Init)> {
    input.keyword("let")?;
    input.keyword("mut")?;
    let (name, _) = input.ident()?;
    input.punct('=')?;
    let (ty, span) = input.ident()?;
    let kind = match ty.as_str() {
        "SparseMap" => Kind::Map,
        "SparseSet" => Kind::Set,
        _ => return Err(Error::at(Some(span), "expected `SparseMap` or `SparseSet`")),
    };
    input.path_separator()?;
    let (constructor, span) = input.ident()?;
    let mut args = input.parenthesized()?;
    let init = match constructor.as_str() {
        "new" => Init::New,
        "with_capacity" => Init::WithCapacity(args.usize()?),
        "from" => {
            let mut items = args.group(Delimiter::Bracket, "an array `[...]`")?;
            let mut entries = Vec::new();
            while !items.is_empty() {
                entries.push(match kind {
                    Kind::Map => {
                        let mut pair = items.parenthesized()?;
                        let key = pair.usize()?;
                        pair.punct(',')?;
                        let value = pair.value(values)?;
                        pair.finish()?;
                        (key, Some(value))
                    }
                    Kind::Set => (items.usize()?, None),
                });
                items.separator()?;
            }
            Init::From(entries)
        }
        _ => {
            return Err(Error::at(
                Some(span),
                "expected `new()`, `with_capacity(n)` or `from([...])`",
            ));
        }
    };
    args.finish()?;
    input.punct(';')?;
    Ok((name, kind, init))
}

/// `<name>.<method>(<args>);`
fn call(input: &mut Cursor<'_>, name: &str, kind: Kind, values: &mut ValueType) -> Result<Call> {
    let (receiver, span) = input.ident()?;
    if receiver != name {
        return Err(Error::at(
            Some(span),
            format!("expected a call on `{name}`"),
        ));
    }
    input.punct('.')?;
    let (method, method_span) = input.ident()?;
    let mut args = input.parenthesized()?;
    let mut texts = Vec::new();
    let mut chained = String::new();
    let op = match (kind, method.as_str()) {
        (Kind::Map, "insert") => {
            let key = key(&mut args, &mut texts)?;
            args.punct(',')?;
            let value = args.value(values)?;
            texts.push(format!("{value:?}"));
            Op::Insert {
                key,
                value: Some(value),
            }
        }
        (Kind::Set, "insert") => Op::Insert {
            key: key(&mut args, &mut texts)?,
            value: None,
        },
        (Kind::Map, "get" | "get_mut") => Op::Get(key(&mut args, &mut texts)?),
        (Kind::Map, "contains_key") | (Kind::Set, "contains") => {
            Op::Contains(key(&mut args, &mut texts)?)
        }
        (Kind::Map, "get_key_value") => Op::GetKeyValue(key(&mut args, &mut texts)?),
        (Kind::Map, "entry") => {
            let key = key(&mut args, &mut texts)?;
            input.punct('.')?;
            input.keyword("or_insert")?;
            let mut inner = input.parenthesized()?;
            let value = inner.value(values)?;
            inner.separator()?;
            inner.finish()?;
            chained = format!(".or_insert({value:?})");
            Op::EntryOrInsert { key, value }
        }
        (_, "get_index_of") => Op::GetIndexOf(key(&mut args, &mut texts)?),
        (_, "get_index") => Op::GetIndex(key(&mut args, &mut texts)?),
        (_, "remove") => Op::Remove(key(&mut args, &mut texts)?),
        (Kind::Map, "remove_entry") => Op::RemoveEntry(key(&mut args, &mut texts)?),
        (_, "swap_remove_index") => Op::SwapRemoveIndex(key(&mut args, &mut texts)?),
        (_, "swap_indices") => {
            let a = key(&mut args, &mut texts)?;
            args.punct(',')?;
            Op::SwapIndices(a, key(&mut args, &mut texts)?)
        }
        (_, "clear") => Op::Clear,
        (_, "retain") => {
            let predicate = predicate(&mut args, kind, values)?;
            texts.push(predicate.text.clone());
            Op::Retain(predicate)
        }
        (_, "reserve") => Op::Reserve(key(&mut args, &mut texts)?),
        (_, "reserve_keys") => Op::ReserveKeys(key(&mut args, &mut texts)?),
        (_, "shrink_to_fit") => Op::ShrinkToFit,
        (Kind::Map, "sort_unstable_keys") | (Kind::Set, "sort_unstable") => Op::Sort,
        _ => {
            let ty = match kind {
                Kind::Map => "SparseMap",
                Kind::Set => "SparseSet",
            };
            return Err(Error::at(
                Some(method_span),
                format!("`{ty}::{method}` is not supported by the diagram model"),
            ));
        }
    };
    args.separator()?;
    args.finish()?;
    input.punct(';')?;
    Ok(Call {
        text: format!("{name}.{method}({}){chained}", texts.join(", ")),
        op,
        span,
    })
}

/// Parses a key or position argument, recording its text.
fn key(args: &mut Cursor<'_>, texts: &mut Vec<String>) -> Result<usize> {
    let key = args.usize()?;
    texts.push(key.to_string());
    Ok(key)
}

/// `|key, value| [*]<binding> [% <int>] <cmp> <literal>`, with one binding
/// for a set.
fn predicate(args: &mut Cursor<'_>, kind: Kind, values: &mut ValueType) -> Result<Predicate> {
    args.punct('|')?;
    let arity = match kind {
        Kind::Map => 2,
        Kind::Set => 1,
    };
    let mut bindings = Vec::new();
    for i in 0..arity {
        if i > 0 {
            args.punct(',')?;
        }
        bindings.push(args.ident()?.0);
    }
    args.punct('|')?;

    let deref = args.eat_punct('*');
    let (binding, span) = args.ident()?;
    let subject = match bindings
        .iter()
        .position(|name| *name == binding && name != "_")
    {
        Some(0) => Subject::Key,
        Some(_) => Subject::Value,
        None => return Err(Error::at(Some(span), "expected a closure parameter")),
    };
    // Keys are passed by value, map values by `&mut`.
    if deref != (subject == Subject::Value) {
        let message = match subject {
            Subject::Key => "keys are passed by value; remove the `*`",
            Subject::Value => "values are passed by reference; write `*value`",
        };
        return Err(Error::at(Some(span), message));
    }
    let modulo_span = args.peek_span();
    let modulo = if args.eat_punct('%') {
        if subject == Subject::Value && matches!(values.0, Some(Value::Char(_))) {
            return Err(Error::at(modulo_span, "`%` needs integer values"));
        }
        Some(args.int()?)
    } else {
        None
    };
    let cmp_span = args.peek_span();
    let cmp = match (args.next_punct(), args.peek_punct()) {
        (Some('='), Some('=')) => Cmp::Eq,
        (Some('!'), Some('=')) => Cmp::Ne,
        (Some('<'), Some('=')) => Cmp::Le,
        (Some('>'), Some('=')) => Cmp::Ge,
        (Some('<'), _) => Cmp::Lt,
        (Some('>'), _) => Cmp::Gt,
        _ => return Err(Error::at(cmp_span, "expected a comparison")),
    };
    if matches!(cmp, Cmp::Eq | Cmp::Ne | Cmp::Le | Cmp::Ge) {
        args.punct('=')?;
    }
    let literal_span = args.peek_span();
    let literal = match subject {
        Subject::Key => Value::Int(args.int()?),
        Subject::Value => args.value(values)?,
    };
    if modulo.is_some() && !matches!(literal, Value::Int(_)) {
        return Err(Error::at(literal_span, "`%` needs integer values"));
    }
    let star = if deref { "*" } else { "" };
    let modulo_text = modulo.map(|m| format!(" % {m}")).unwrap_or_default();
    Ok(Predicate {
        text: format!(
            "|{}| {star}{binding}{modulo_text} {} {literal:?}",
            bindings.join(", "),
            cmp.symbol()
        ),
        subject,
        modulo,
        cmp,
        literal,
    })
}

/// The type of the first value literal; later ones must match it.
#[derive(Default)]
struct ValueType(Option<Value>);

impl ValueType {
    fn check(&mut self, value: Value, span: Option<usize>) -> Result<Value> {
        match self.0 {
            Some(first) if first.kind_name() != value.kind_name() => Err(Error::at(
                span,
                format!("expected a {} value, like the others", first.kind_name()),
            )),
            Some(_) => Ok(value),
            None => {
                self.0 = Some(value);
                Ok(value)
            }
        }
    }
}

/// A position in a token list. `end` is the span of the enclosing group, for
/// errors at its end.
struct Cursor<'a> {
    trees: &'a [Tree],
    position: usize,
    end: Option<usize>,
}

impl<'a> Cursor<'a> {
    fn new(trees: &'a [Tree], end: Option<usize>) -> Self {
        Self {
            trees,
            position: 0,
            end,
        }
    }

    fn is_empty(&self) -> bool {
        self.position == self.trees.len()
    }

    fn peek(&self) -> Option<&'a Tree> {
        self.trees.get(self.position)
    }

    fn peek_span(&self) -> Option<usize> {
        self.peek().map(Tree::span).or(self.end)
    }

    fn error(&self, expected: &str) -> Error {
        let message = match self.peek() {
            Some(_) => format!("expected {expected}"),
            None => format!("expected {expected}, found the end of the input"),
        };
        Error::at(self.peek_span(), message)
    }

    fn ident(&mut self) -> Result<(String, usize)> {
        match self.peek() {
            Some(Tree::Ident { name, span }) => {
                self.position += 1;
                Ok((name.clone(), *span))
            }
            _ => Err(self.error("an identifier")),
        }
    }

    fn keyword(&mut self, keyword: &str) -> Result<()> {
        match self.peek() {
            Some(Tree::Ident { name, .. }) if name == keyword => {
                self.position += 1;
                Ok(())
            }
            _ => Err(self.error(&format!("`{keyword}`"))),
        }
    }

    fn peek_punct(&self) -> Option<char> {
        match self.peek() {
            Some(&Tree::Punct { ch, .. }) => Some(ch),
            _ => None,
        }
    }

    fn next_punct(&mut self) -> Option<char> {
        let ch = self.peek_punct()?;
        self.position += 1;
        Some(ch)
    }

    fn eat_punct(&mut self, ch: char) -> bool {
        let found = self.peek_punct() == Some(ch);
        if found {
            self.position += 1;
        }
        found
    }

    fn punct(&mut self, ch: char) -> Result<()> {
        if self.eat_punct(ch) {
            Ok(())
        } else {
            Err(self.error(&format!("`{ch}`")))
        }
    }

    fn path_separator(&mut self) -> Result<()> {
        if self.eat_punct(':') && self.eat_punct(':') {
            Ok(())
        } else {
            Err(self.error("`::`"))
        }
    }

    /// Consumes a `,` unless at the end.
    fn separator(&mut self) -> Result<()> {
        if self.is_empty() {
            Ok(())
        } else {
            self.punct(',')
        }
    }

    fn finish(&self) -> Result<()> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(self.error("no more arguments"))
        }
    }

    fn group(&mut self, delimiter: Delimiter, expected: &str) -> Result<Cursor<'a>> {
        match self.peek() {
            Some(Tree::Group {
                delimiter: found,
                trees,
                span,
            }) if *found == delimiter => {
                self.position += 1;
                Ok(Cursor::new(trees, Some(*span)))
            }
            _ => Err(self.error(expected)),
        }
    }

    fn parenthesized(&mut self) -> Result<Cursor<'a>> {
        self.group(Delimiter::Parenthesis, "`(...)`")
    }

    fn literal(&mut self, expected: &str) -> Result<(&'a str, usize)> {
        match self.peek() {
            Some(Tree::Literal { text, span }) => {
                self.position += 1;
                Ok((text, *span))
            }
            _ => Err(self.error(expected)),
        }
    }

    fn usize(&mut self) -> Result<usize> {
        let (text, span) = self.literal("an integer literal")?;
        integer(text)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| Error::at(Some(span), "expected a non-negative integer literal"))
    }

    fn int(&mut self) -> Result<i64> {
        let negative = self.eat_punct('-');
        let (text, span) = self.literal("an integer literal")?;
        let n =
            integer(text).ok_or_else(|| Error::at(Some(span), "expected an integer literal"))?;
        Ok(if negative { -n } else { n })
    }

    /// A `char` or integer literal of the same type as the others.
    fn value(&mut self, values: &mut ValueType) -> Result<Value> {
        let span = self.peek_span();
        let value = self.any_value()?;
        values.check(value, span)
    }

    fn any_value(&mut self) -> Result<Value> {
        if let Some(Tree::Literal { text, span }) = self.peek()
            && text.starts_with('\'')
        {
            self.position += 1;
            return character(text)
                .map(Value::Char)
                .ok_or_else(|| Error::at(Some(*span), "unsupported char literal"));
        }
        if matches!(
            self.peek(),
            Some(Tree::Literal { .. } | Tree::Punct { ch: '-', .. })
        ) {
            return self.int().map(Value::Int);
        }
        Err(self.error("a char or integer literal"))
    }
}

/// Parses an integer literal with an optional integer suffix.
fn integer(text: &str) -> Option<i64> {
    const SUFFIXES: [&str; 12] = [
        "usize", "isize", "u128", "i128", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8",
    ];
    let digits = SUFFIXES
        .iter()
        .find_map(|suffix| text.strip_suffix(suffix))
        .unwrap_or(text);
    if digits.is_empty() || !digits.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    digits.replace('_', "").parse().ok()
}

/// Parses a char literal: one character or a simple escape.
fn character(text: &str) -> Option<char> {
    let inner = text.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut chars = inner.chars();
    let c = match chars.next()? {
        '\\' => match chars.next()? {
            'n' => '\n',
            't' => '\t',
            '0' => '\0',
            c @ ('\\' | '\'' | '"') => c,
            _ => return None,
        },
        c => c,
    };
    chars.next().is_none().then_some(c)
}
