use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::num::NonZeroU32;

use crate::dense::MAX_LEN;

const MIN_SLOTS: usize = 64;

/// Slots per chunk: one 4 KiB page.
const CHUNK_SHIFT: u32 = 10;

/// Maps key slots to dense indices.
///
/// Fresh slots come from zeroed allocations, which system allocators back with
/// lazily mapped pages, so untouched slots cost no memory.
///
/// One flag per page-sized chunk of slots records whether the chunk may hold an
/// occupied slot. Insertion writes into a clear chunk without reading it first,
/// so a fresh page takes one write fault instead of a read fault followed by a
/// copy-on-write fault. Slots below `flagged` lie in flagged chunks, so
/// insertion there skips the flags entirely.
///
/// # Invariants
///
/// * `chunks.len() == slots.len().div_ceil(1 << CHUNK_SHIFT)`.
/// * If a slot is occupied, its chunk flag is set.
/// * `flagged <= slots.len()`, and every chunk below `flagged` is flagged.
#[derive(Default)]
pub(crate) struct Sparse {
    slots: Vec<Slot>,
    chunks: Vec<bool>,
    flagged: usize,
}

/// A dense index stored as `index + 1`, so an empty slot is all zero bits.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub(crate) struct Slot(Option<NonZeroU32>);

impl Slot {
    /// Returns the stored index if it is below `len`.
    ///
    /// Testing for an empty slot first keeps misses to one branch.
    #[inline]
    pub(crate) fn get(self, len: usize) -> Option<usize> {
        let index = self.0?.get() as usize - 1;
        (index < len).then_some(index)
    }

    #[inline]
    fn is_occupied(self) -> bool {
        self.0.is_some()
    }

    /// Stores `index`, which must be below `MAX_LEN`.
    #[inline]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "indices are below `MAX_LEN`"
    )]
    fn set(&mut self, index: usize) {
        debug_assert!(index < MAX_LEN);
        self.0 = Some(NonZeroU32::MIN.saturating_add(index as u32));
    }

    /// Stores `index`.
    ///
    /// # Safety
    ///
    /// `index` must be below `MAX_LEN`.
    #[inline]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "indices are below `MAX_LEN`"
    )]
    pub(crate) unsafe fn set_unchecked(&mut self, index: usize) {
        debug_assert!(index < MAX_LEN);
        // SAFETY: guaranteed by the caller; `index + 1` cannot wrap to zero.
        self.0 = Some(unsafe { NonZeroU32::new_unchecked(index as u32 + 1) });
    }

    /// Empties the slot, returning the stored index if it is below `len`.
    #[inline]
    fn take(&mut self, len: usize) -> Option<usize> {
        Self(self.0.take()).get(len)
    }
}

impl Sparse {
    pub(crate) const fn new() -> Self {
        Self {
            slots: Vec::new(),
            chunks: Vec::new(),
            flagged: 0,
        }
    }

    fn zeroed(len: usize) -> Self {
        // SAFETY: `Slot` is a transparent `Option<NonZeroU32>`, whose all-zero
        // value is `None`.
        let slots = unsafe { Box::new_zeroed_slice(len).assume_init() }.into_vec();
        Self {
            slots,
            chunks: vec![false; chunks(len)],
            flagged: 0,
        }
    }

    /// Builds `len` slots for the live key slots, given in dense order.
    fn from_live(len: usize, live: impl Iterator<Item = usize>) -> Self {
        let mut sparse = Self::zeroed(len);
        for (index, slot) in live.enumerate() {
            sparse.set(slot, index);
        }
        sparse
    }

    /// Clones `self`, touching only live slots when they are few.
    pub(crate) fn clone_with(&self, live: impl ExactSizeIterator<Item = usize>) -> Self {
        if is_crowded(live.len(), self.slots.len()) {
            Self {
                slots: self.slots.clone(),
                chunks: self.chunks.clone(),
                flagged: self.flagged,
            }
        } else {
            Self::from_live(self.slots.len(), live)
        }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.slots.len()
    }

    /// Returns the index stored in `slot` if it is below `len`.
    #[inline]
    pub(crate) fn get(&self, slot: usize, len: usize) -> Option<usize> {
        self.slots.get(slot)?.get(len)
    }

    /// Returns `true` if `slot` is occupied.
    #[inline]
    pub(crate) fn contains(&self, slot: usize) -> bool {
        self.slots.get(slot).is_some_and(|slot| slot.is_occupied())
    }

    /// Returns `slot` if its chunk is known to be flagged.
    #[inline]
    pub(crate) fn flagged(&mut self, slot: usize) -> Option<&mut Slot> {
        if slot < self.flagged {
            // SAFETY: `flagged <= slots.len()`.
            Some(unsafe { self.slots.get_unchecked_mut(slot) })
        } else {
            None
        }
    }

    /// Returns the index stored in `slot`, if below `len`, without reading
    /// a slot in a clear chunk.
    #[inline]
    pub(crate) fn probe(&self, slot: usize, len: usize) -> Option<usize> {
        let &flag = self.chunks.get(slot >> CHUNK_SHIFT)?;
        if flag { self.get(slot, len) } else { None }
    }

    /// Ensures `slot` exists.
    #[inline]
    pub(crate) fn reserve(&mut self, slot: usize, live: impl ExactSizeIterator<Item = usize>) {
        if slot >= self.slots.len() {
            self.grow(slot, live);
        }
    }

    #[cold]
    #[inline(never)]
    fn grow(&mut self, slot: usize, live: impl ExactSizeIterator<Item = usize>) {
        let len = slot
            .checked_add(1)
            .expect("key slot overflow")
            .max(self.slots.len() * 2)
            .max(MIN_SLOTS);
        if is_crowded(live.len(), self.slots.len()) {
            let mut grown = Self::zeroed(len);
            grown.slots[..self.slots.len()].copy_from_slice(&self.slots);
            grown.chunks[..self.chunks.len()].copy_from_slice(&self.chunks);
            grown.flagged = self.flagged;
            grown.advance();
            *self = grown;
        } else {
            *self = Self::from_live(len, live);
        }
    }

    /// Points `slot` at `index`, which must be below `MAX_LEN`, flagging its
    /// chunk. Does nothing if `slot` does not exist.
    #[inline]
    pub(crate) fn set(&mut self, slot: usize, index: usize) {
        if let Some(entry) = self.slots.get_mut(slot) {
            entry.set(index);
            self.flag(slot);
        }
    }

    /// Points `slot` at `index`, flagging its chunk.
    ///
    /// # Safety
    ///
    /// `slot` must exist and `index` must be below `MAX_LEN`.
    #[inline]
    pub(crate) unsafe fn set_unchecked(&mut self, slot: usize, index: usize) {
        // SAFETY: guaranteed by the caller.
        unsafe { self.slots.get_unchecked_mut(slot).set_unchecked(index) };
        self.flag(slot);
    }

    /// Points the occupied `slot` at `index`, which must be below `MAX_LEN`.
    /// An occupied slot's chunk is already flagged.
    #[inline]
    pub(crate) fn repoint(&mut self, slot: usize, index: usize) {
        if let Some(entry) = self.slots.get_mut(slot) {
            entry.set(index);
        }
    }

    /// Flags the chunk of the existing `slot` and moves `flagged` past the
    /// following flagged chunks.
    fn flag(&mut self, slot: usize) {
        self.chunks[slot >> CHUNK_SHIFT] = true;
        self.advance();
    }

    /// Moves `flagged` past the following flagged chunks.
    fn advance(&mut self) {
        let mut chunk = self.flagged >> CHUNK_SHIFT;
        while self.chunks.get(chunk).is_some_and(|&flag| flag) {
            chunk += 1;
        }
        self.flagged = (chunk << CHUNK_SHIFT).min(self.slots.len());
    }

    /// Empties `slot`, returning its index if it is below `len`.
    #[inline]
    pub(crate) fn take(&mut self, slot: usize, len: usize) -> Option<usize> {
        self.slots.get_mut(slot)?.take(len)
    }

    #[inline]
    pub(crate) fn remove(&mut self, slot: usize) {
        if let Some(entry) = self.slots.get_mut(slot) {
            *entry = Slot::default();
        }
    }

    /// Empties the live key slots.
    pub(crate) fn remove_all(&mut self, live: impl ExactSizeIterator<Item = usize>) {
        if is_crowded(live.len(), self.slots.len()) {
            self.slots.fill(Slot::default());
            self.chunks.fill(true);
            self.flagged = self.slots.len();
        } else {
            live.for_each(|slot| self.remove(slot));
        }
    }

    /// Releases slots at and beyond `end`.
    pub(crate) fn shrink_to(&mut self, end: usize) {
        self.slots.truncate(end);
        self.slots.shrink_to_fit();
        self.chunks.truncate(chunks(end));
        self.chunks.shrink_to_fit();
        self.flagged = self.flagged.min(self.slots.len());
    }
}

/// Chunks covering `slots` slots.
fn chunks(slots: usize) -> usize {
    slots.div_ceil(1 << CHUNK_SHIFT)
}

/// Whether sweeping all slots beats touching each live one: a scattered
/// store costs roughly eight sequential slot writes.
fn is_crowded(live: usize, slots: usize) -> bool {
    live.saturating_mul(8) >= slots
}
