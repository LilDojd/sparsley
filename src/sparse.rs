use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::num::NonZeroU32;

use crate::dense::MAX_LEN;

const MIN_SLOTS: usize = 64;

/// Slots per chunk: one 4 KiB page.
const CHUNK_SHIFT: u32 = 10;

/// Key index to dense position map.
///
/// A slot stores `position + 1`, so an empty slot is all zero bits and fresh
/// slots come from zeroed allocations. System allocators back large zeroed
/// allocations with lazily mapped pages, so untouched slots cost no memory.
///
/// One flag per page-sized chunk of slots records whether the chunk may hold a
/// non-empty slot. Insertion writes into a clear chunk without reading it
/// first, so a fresh page takes one write fault instead of a read fault
/// followed by a copy-on-write fault.
///
/// # Invariants
///
/// * `chunks.len() == slots.len().div_ceil(1 << CHUNK_SHIFT)`.
/// * If a slot is non-empty, its chunk flag is set.
#[derive(Default)]
pub(crate) struct Sparse {
    slots: Vec<Option<NonZeroU32>>,
    chunks: Vec<bool>,
}

impl Sparse {
    pub(crate) const fn new() -> Self {
        Self {
            slots: Vec::new(),
            chunks: Vec::new(),
        }
    }

    fn zeroed(len: usize) -> Self {
        // SAFETY: the all-zero `Option<NonZeroU32>` is `None`.
        let slots = unsafe { Box::new_zeroed_slice(len).assume_init() }.into_vec();
        Self {
            slots,
            chunks: vec![false; chunks(len)],
        }
    }

    /// Builds `len` slots for the live key indices, given in dense order.
    fn from_live(len: usize, live: impl Iterator<Item = usize>) -> Self {
        let mut sparse = Self::zeroed(len);
        for (position, index) in live.enumerate() {
            sparse.set(index, position);
        }
        sparse
    }

    /// Clones `self`, touching only live slots when they are few.
    pub(crate) fn clone_with(&self, live: impl ExactSizeIterator<Item = usize>) -> Self {
        if is_crowded(live.len(), self.slots.len()) {
            Self {
                slots: self.slots.clone(),
                chunks: self.chunks.clone(),
            }
        } else {
            Self::from_live(self.slots.len(), live)
        }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.slots.len()
    }

    /// Returns the position stored at `index` if it is below `len`.
    #[inline]
    pub(crate) fn position(&self, index: usize, len: usize) -> Option<usize> {
        checked(*self.slots.get(index)?, len)
    }

    /// Returns the slot of `index` for writing.
    #[inline]
    pub(crate) fn slot_mut(&mut self, index: usize) -> Option<SlotMut<'_>> {
        let slot = self.slots.get_mut(index)?;
        // SAFETY: `index` has a slot, so its chunk exists by the invariant.
        let chunk = unsafe { self.chunks.get_unchecked_mut(index >> CHUNK_SHIFT) };
        let clear = !*chunk;
        Some(SlotMut { slot, chunk, clear })
    }

    /// Ensures `index` has a slot.
    #[inline]
    pub(crate) fn reserve(&mut self, index: usize, live: impl ExactSizeIterator<Item = usize>) {
        if index >= self.slots.len() {
            self.grow(index, live);
        }
    }

    #[cold]
    #[inline(never)]
    fn grow(&mut self, index: usize, live: impl ExactSizeIterator<Item = usize>) {
        let len = index
            .checked_add(1)
            .expect("key index overflow")
            .max(self.slots.len() * 2)
            .max(MIN_SLOTS);
        if is_crowded(live.len(), self.slots.len()) {
            let mut grown = Self::zeroed(len);
            grown.slots[..self.slots.len()].copy_from_slice(&self.slots);
            grown.chunks[..self.chunks.len()].copy_from_slice(&self.chunks);
            *self = grown;
        } else {
            *self = Self::from_live(len, live);
        }
    }

    /// Points `index` at `position`, which must be below `MAX_LEN`. Does
    /// nothing if `index` has no slot.
    #[inline]
    pub(crate) fn set(&mut self, index: usize, position: usize) {
        if let Some(slot) = self.slot_mut(index) {
            slot.set(encode(position));
        }
    }

    /// Points `index` at `position`.
    ///
    /// # Safety
    ///
    /// `index` must have a slot and `position` must be below `MAX_LEN`.
    #[inline]
    pub(crate) unsafe fn set_unchecked(&mut self, index: usize, position: usize) {
        debug_assert!(index < self.slots.len());
        // SAFETY: guaranteed by the caller.
        unsafe {
            self.slot_mut(index)
                .unwrap_unchecked()
                .set_unchecked(position);
        }
    }

    /// Empties `index`, returning its position if it is below `len`.
    #[inline]
    pub(crate) fn take(&mut self, index: usize, len: usize) -> Option<usize> {
        checked(self.slots.get_mut(index)?.take(), len)
    }

    /// Returns `true` if `index` holds a position.
    #[inline]
    pub(crate) fn contains(&self, index: usize) -> bool {
        matches!(self.slots.get(index), Some(Some(_)))
    }

    /// Points the live slot of `index` at `position`, which must be below
    /// `MAX_LEN`. A live slot already has its chunk flag set.
    #[inline]
    pub(crate) fn repoint(&mut self, index: usize, position: usize) {
        if let Some(slot) = self.slots.get_mut(index) {
            *slot = Some(encode(position));
        }
    }

    #[inline]
    pub(crate) fn remove(&mut self, index: usize) {
        if let Some(slot) = self.slots.get_mut(index) {
            *slot = None;
        }
    }

    /// Empties the slots of the live key indices.
    pub(crate) fn remove_all(&mut self, live: impl ExactSizeIterator<Item = usize>) {
        if is_crowded(live.len(), self.slots.len()) {
            self.slots.fill(None);
            self.chunks.fill(false);
        } else {
            live.for_each(|index| self.remove(index));
        }
    }

    /// Releases slots at and beyond `end`.
    pub(crate) fn shrink_to(&mut self, end: usize) {
        self.slots.truncate(end);
        self.slots.shrink_to_fit();
        self.chunks.truncate(chunks(end));
        self.chunks.shrink_to_fit();
    }
}

/// An existing sparse slot and its chunk flag.
pub(crate) struct SlotMut<'a> {
    slot: &'a mut Option<NonZeroU32>,
    chunk: &'a mut bool,
    clear: bool,
}

impl SlotMut<'_> {
    /// Returns the stored position if it is below `len`.
    #[inline]
    pub(crate) fn position(&self, len: usize) -> Option<usize> {
        if self.clear {
            return None;
        }
        checked(*self.slot, len)
    }

    #[inline]
    fn set(self, slot: NonZeroU32) {
        if self.clear {
            *self.chunk = true;
        }
        *self.slot = Some(slot);
    }

    /// Points the slot at `position`.
    ///
    /// # Safety
    ///
    /// `position` must be below `MAX_LEN`.
    #[inline]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "positions are below `MAX_LEN`"
    )]
    pub(crate) unsafe fn set_unchecked(self, position: usize) {
        debug_assert!(position < MAX_LEN);
        // SAFETY: guaranteed by the caller; `position + 1` cannot wrap to zero.
        self.set(unsafe { NonZeroU32::new_unchecked(position as u32 + 1) });
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

#[inline]
#[allow(
    clippy::cast_possible_truncation,
    reason = "positions are below `MAX_LEN`"
)]
fn encode(position: usize) -> NonZeroU32 {
    debug_assert!(position < MAX_LEN);
    NonZeroU32::MIN.saturating_add(position as u32)
}

/// Decodes `slot` if it holds a position below `len`.
///
/// Testing for an empty slot first keeps misses to one branch.
#[inline]
fn checked(slot: Option<NonZeroU32>, len: usize) -> Option<usize> {
    let position = slot?.get() as usize - 1;
    (position < len).then_some(position)
}
