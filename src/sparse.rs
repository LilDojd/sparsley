use alloc::boxed::Box;
use alloc::vec::Vec;
use core::num::NonZeroU32;

use crate::dense::MAX_LEN;

const MIN_SLOTS: usize = 64;

/// Key index to dense position map.
///
/// A slot stores `position + 1`, so an empty slot is all zero bits and fresh
/// slots come from zeroed allocations. System allocators back large zeroed
/// allocations with lazily mapped pages, so untouched slots cost no memory.
#[derive(Default)]
pub(crate) struct Sparse {
    slots: Vec<Option<NonZeroU32>>,
}

impl Sparse {
    pub(crate) const fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Builds `len` slots for the live key indices, given in dense order.
    fn from_live(len: usize, live: impl Iterator<Item = usize>) -> Self {
        let mut sparse = Self { slots: zeroed(len) };
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
        let position = decode(*self.slots.get(index)?);
        (position < len).then_some(position)
    }

    #[inline]
    pub(crate) fn contains(&self, index: usize) -> bool {
        matches!(self.slots.get(index), Some(Some(_)))
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
            let mut slots = zeroed(len);
            slots[..self.slots.len()].copy_from_slice(&self.slots);
            self.slots = slots;
        } else {
            *self = Self::from_live(len, live);
        }
    }

    /// Points `index` at `position`, which must be below `MAX_LEN`. Does
    /// nothing if `index` has no slot.
    #[inline]
    pub(crate) fn set(&mut self, index: usize, position: usize) {
        if let Some(slot) = self.slots.get_mut(index) {
            *slot = Some(encode(position));
        }
    }

    /// Points `index` at `position`.
    ///
    /// # Safety
    ///
    /// `index` must have a slot and `position` must be below `MAX_LEN`.
    #[inline]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "positions are below `MAX_LEN`"
    )]
    pub(crate) unsafe fn set_unchecked(&mut self, index: usize, position: usize) {
        debug_assert!(index < self.slots.len() && position < MAX_LEN);
        // SAFETY: guaranteed by the caller; `position + 1` cannot wrap to zero.
        unsafe {
            *self.slots.get_unchecked_mut(index) =
                Some(NonZeroU32::new_unchecked(position as u32 + 1));
        }
    }

    /// Empties `index`, returning its position if it is below `len`.
    #[inline]
    pub(crate) fn take(&mut self, index: usize, len: usize) -> Option<usize> {
        let position = decode(self.slots.get_mut(index)?.take());
        (position < len).then_some(position)
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
        } else {
            live.for_each(|index| self.remove(index));
        }
    }

    /// Releases slots at and beyond `end`.
    pub(crate) fn shrink_to(&mut self, end: usize) {
        self.slots.truncate(end);
        self.slots.shrink_to_fit();
    }
}

/// Whether sweeping all slots beats touching each live one: a scattered
/// store costs roughly eight sequential slot writes.
fn is_crowded(live: usize, slots: usize) -> bool {
    live.saturating_mul(8) >= slots
}

fn zeroed(len: usize) -> Vec<Option<NonZeroU32>> {
    // SAFETY: the all-zero `Option<NonZeroU32>` is `None`.
    unsafe { Box::new_zeroed_slice(len).assume_init() }.into_vec()
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

/// Empty slots decode to `usize::MAX`, which is never a valid position.
#[inline]
fn decode(slot: Option<NonZeroU32>) -> usize {
    (slot.map_or(0, NonZeroU32::get) as usize).wrapping_sub(1)
}
