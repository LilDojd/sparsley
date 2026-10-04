//! A reference model of the observable behavior of `SparseMap` and
//! `SparseSet` with `usize` keys, following the library's growth policies.

use crate::parse::{Call, Error, Init, Op, Predicate, Program, Result, Subject, Value};
use crate::render::{self, COLUMNS};
use crate::snapshot::{Entry, Kind, Slot, Snapshot};

/// Sparse arrays never grow to fewer slots.
const MIN_SLOTS: usize = 64;

/// Dense arrays never grow to fewer entries.
const MIN_CAPACITY: usize = 4;

/// The model state.
#[derive(Clone, Debug)]
pub(crate) struct Model {
    kind: Kind,
    /// Keys and values in dense order; set entries have no value.
    entries: Vec<(usize, Option<Value>)>,
    capacity: usize,
    key_capacity: usize,
}

/// What an insertion did.
enum Insertion {
    Added,
    /// Replaced the value of a present key; sets hold no value.
    Replaced(Option<Value>),
}

/// The effect of one call.
pub(crate) struct Step<'a> {
    call: &'a Call,
    /// The `Debug` text of the return value.
    result: String,
    /// The key index the call looked up, if any.
    focus: Option<usize>,
    before: Snapshot,
    after: Snapshot,
}

/// A program run on the model.
pub(crate) struct Run<'a> {
    initial: Model,
    steps: Vec<Step<'a>>,
    last: Model,
}

impl Model {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            entries: Vec::new(),
            capacity: 0,
            key_capacity: 0,
        }
    }

    fn init(kind: Kind, init: &Init) -> Self {
        let mut model = Self::new(kind);
        match init {
            Init::New => {}
            Init::WithCapacity(capacity) => model.reserve(*capacity),
            Init::From(entries) => {
                model.reserve(entries.len());
                for &(key, value) in entries {
                    model.insert(key, value);
                }
            }
        }
        model
    }

    fn position(&self, key: usize) -> Option<usize> {
        self.entries.iter().position(|&(k, _)| k == key)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    /// Ensures room for `additional` more entries, at least doubling.
    fn reserve(&mut self, additional: usize) {
        if additional > self.capacity - self.len() {
            self.capacity = (self.len() + additional).max(self.capacity * 2);
        }
    }

    /// Ensures `key` has a sparse slot.
    fn reserve_slot(&mut self, key: usize) {
        if key >= self.key_capacity {
            self.key_capacity = (key + 1).max(self.key_capacity * 2).max(MIN_SLOTS);
        }
    }

    fn insert(&mut self, key: usize, value: Option<Value>) -> Insertion {
        self.reserve_slot(key);
        if let Some(position) = self.position(key) {
            return Insertion::Replaced(std::mem::replace(&mut self.entries[position].1, value));
        }
        if self.len() == self.capacity {
            self.capacity = (self.capacity * 2).max(MIN_CAPACITY);
        }
        self.entries.push((key, value));
        Insertion::Added
    }

    fn keep(predicate: &Predicate, (key, value): (usize, Option<Value>)) -> bool {
        let subject = match predicate.subject {
            Subject::Key => Value::Int(i64::try_from(key).expect("key fits in i64")),
            Subject::Value => value.expect("set predicates compare keys"),
        };
        match (subject, predicate.modulo) {
            (Value::Int(n), Some(m)) => predicate.cmp.holds(Value::Int(n % m), predicate.literal),
            _ => predicate.cmp.holds(subject, predicate.literal),
        }
    }

    /// Applies `op`, returning the `Debug` text of its result and the key
    /// index it looked up.
    fn apply(&mut self, op: &Op) -> std::result::Result<(String, Option<usize>), String> {
        if let Some(lookup) = self.look_up(op) {
            return Ok(lookup);
        }
        let unit = || ("()".to_owned(), None);
        let set = self.kind == Kind::Set;
        Ok(match *op {
            Op::Insert { key, value } => {
                let result = match (self.insert(key, value), set) {
                    (Insertion::Added, true) => "true".to_owned(),
                    (Insertion::Replaced(_), true) => "false".to_owned(),
                    (Insertion::Added, false) => "None".to_owned(),
                    (Insertion::Replaced(old), false) => some(Some(value_text(old))),
                };
                (result, Some(key))
            }
            Op::EntryOrInsert { key, value } => {
                self.reserve_slot(key);
                let current = if let Some(position) = self.position(key) {
                    self.entries[position].1
                } else {
                    self.insert(key, Some(value));
                    Some(value)
                };
                (value_text(current), Some(key))
            }
            Op::Remove(key) => {
                let removed = self.position(key).map(|p| self.entries.swap_remove(p));
                let result = if set {
                    removed.is_some().to_string()
                } else {
                    some(removed.map(|(_, value)| value_text(value)))
                };
                (result, Some(key))
            }
            Op::RemoveEntry(key) => {
                let removed = self.position(key).map(|p| self.entries.swap_remove(p));
                (some(removed.map(entry_text)), Some(key))
            }
            Op::SwapRemoveIndex(index) => {
                let focus = self.entries.get(index).map(|&(key, _)| key);
                let removed = (index < self.len()).then(|| self.entries.swap_remove(index));
                (some(removed.map(entry_text)), focus)
            }
            Op::SwapIndices(a, b) => {
                if a >= self.len() || b >= self.len() {
                    return Err(format!("index out of bounds: the length is {}", self.len()));
                }
                self.entries.swap(a, b);
                unit()
            }
            Op::Clear => {
                self.entries.clear();
                unit()
            }
            Op::Retain(ref predicate) => {
                for index in (0..self.len()).rev() {
                    if !Self::keep(predicate, self.entries[index]) {
                        self.entries.swap_remove(index);
                    }
                }
                unit()
            }
            Op::Reserve(additional) => {
                self.reserve(additional);
                unit()
            }
            Op::ReserveKeys(end) => {
                if let Some(last) = end.checked_sub(1) {
                    self.reserve_slot(last);
                }
                unit()
            }
            Op::ShrinkToFit => {
                self.capacity = self.len();
                let end = self.entries.iter().map(|&(key, _)| key + 1).max();
                self.key_capacity = self.key_capacity.min(end.unwrap_or(0));
                unit()
            }
            Op::Sort => {
                self.entries.sort_unstable_by_key(|&(key, _)| key);
                unit()
            }
            Op::Get(_)
            | Op::GetKeyValue(_)
            | Op::Contains(_)
            | Op::GetIndexOf(_)
            | Op::GetIndex(_) => unreachable!("lookups are handled by `look_up`"),
        })
    }

    /// Returns the result and focus of a call that changes nothing.
    fn look_up(&self, op: &Op) -> Option<(String, Option<usize>)> {
        let found = |key| self.position(key).map(|index| self.entries[index]);
        Some(match *op {
            Op::Get(key) => (
                some(found(key).map(|(_, value)| value_text(value))),
                Some(key),
            ),
            Op::GetKeyValue(key) => (some(found(key).map(entry_text)), Some(key)),
            Op::Contains(key) => (found(key).is_some().to_string(), Some(key)),
            Op::GetIndexOf(key) => (some(self.position(key).map(|i| i.to_string())), Some(key)),
            Op::GetIndex(index) => {
                let entry = self.entries.get(index).copied();
                (some(entry.map(entry_text)), entry.map(|(key, _)| key))
            }
            _ => return None,
        })
    }

    pub(crate) fn snapshot(&self) -> Snapshot {
        Snapshot {
            kind: self.kind,
            slots: (0..self.key_capacity.min(COLUMNS))
                .map(|key| Slot::from(self.position(key)))
                .collect(),
            key_capacity: self.key_capacity,
            entries: self
                .entries
                .iter()
                .map(|&(key, value)| Entry {
                    index: key,
                    key: key.to_string(),
                    value: value.map(|value| value.to_string()),
                })
                .collect(),
            capacity: self.capacity,
        }
    }
}

impl<'a> Run<'a> {
    /// Runs `program`, failing at a call the library would panic on.
    pub(crate) fn new(program: &'a Program) -> Result<Self> {
        let initial = Model::init(program.kind, &program.init);
        let mut model = initial.clone();
        let mut steps = Vec::new();
        for call in &program.calls {
            let before = model.snapshot();
            let (result, focus) = model.apply(&call.op).map_err(|message| Error {
                message,
                span: Some(call.span),
            })?;
            steps.push(Step {
                call,
                result,
                focus,
                before,
                after: model.snapshot(),
            });
        }
        Ok(Self {
            initial,
            steps,
            last: model,
        })
    }

    /// Draws each call, or the initial state if there are none, as a fenced
    /// `text` block.
    pub(crate) fn diagram(&self) -> String {
        let body = if self.steps.is_empty() {
            render::layout(&self.initial.snapshot())
        } else {
            let steps: Vec<String> = self
                .steps
                .iter()
                .map(|step| {
                    render::render(
                        &step.call.text,
                        &step.result,
                        &step.before,
                        &step.after,
                        step.focus,
                    )
                })
                .collect();
            steps.join("\n")
        };
        format!("```text\n{body}```")
    }

    /// Returns Rust source for the final state: slots up to `key_capacity`,
    /// keys, values, `len`, `capacity` and `key_capacity`.
    pub(crate) fn state(&self, value_type: &str) -> String {
        let model = &self.last;
        let slots: Vec<String> = (0..model.key_capacity)
            .map(|key| match model.position(key) {
                Some(position) => format!("::core::option::Option::Some({position})"),
                None => "::core::option::Option::None".to_owned(),
            })
            .collect();
        let keys: Vec<String> = model
            .entries
            .iter()
            .map(|&(key, _)| key.to_string())
            .collect();
        let values: Vec<String> = model
            .entries
            .iter()
            .filter_map(|&(_, value)| value.map(|value| format!("{value:?}")))
            .collect();
        format!(
            "{{ let slots: &[::core::option::Option<usize>] = &[{}]; let keys: &[usize] = &[{}]; \
             let values: &[{value_type}] = &[{}]; (slots, keys, values, {}usize, {}usize, {}usize) }}",
            slots.join(", "),
            keys.join(", "),
            values.join(", "),
            model.len(),
            model.capacity,
            model.key_capacity,
        )
    }
}

/// Formats a map value as `Debug` prints it.
fn value_text(value: Option<Value>) -> String {
    format!("{:?}", value.expect("map entries have values"))
}

/// Formats an entry as `Debug` prints `(K, V)`, or a set key as `K`.
fn entry_text((key, value): (usize, Option<Value>)) -> String {
    match value {
        Some(value) => format!("({key}, {value:?})"),
        None => key.to_string(),
    }
}

/// Formats `Option<T>` as `Debug` prints it, given the text of `T`.
fn some(text: Option<String>) -> String {
    text.map_or_else(|| "None".to_owned(), |text| format!("Some({text})"))
}
