use alloc::vec;
use alloc::vec::Vec;
use core::num::NonZeroU32;

use crate::dense::MAX_LEN;

const MIN_SLOTS: usize = 64;

/// Key index to dense position map.
///
/// A slot stores `position + 1`, so an empty slot is all zero bits. Fresh
/// slots come from zeroed allocations, which the OS maps lazily.
#[derive(Default)]
pub(crate) struct Sparse {
    slots: Vec<Option<NonZeroU32>>,
}

impl Clone for Sparse {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.slots.clone_from(&source.slots);
    }
}

impl Sparse {
    pub(crate) const fn new() -> Self {
        Self { slots: Vec::new() }
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
    pub(crate) fn reserve(&mut self, index: usize) {
        if index >= self.slots.len() {
            self.grow(index);
        }
    }

    #[cold]
    #[inline(never)]
    fn grow(&mut self, index: usize) {
        let len = index
            .checked_add(1)
            .expect("key index overflow")
            .max(self.slots.len() * 2)
            .max(MIN_SLOTS);
        let mut slots = vec![None; len];
        slots[..self.slots.len()].copy_from_slice(&self.slots);
        self.slots = slots;
    }

    /// Points `index` at `position`, which must be below `MAX_LEN`.
    ///
    /// # Panics
    ///
    /// Panics if `index` has no slot.
    #[inline]
    pub(crate) fn set(&mut self, index: usize, position: usize) {
        self.slots[index] = Some(encode(position));
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

    /// Empties the slots of `indices`.
    pub(crate) fn remove_all(&mut self, indices: impl ExactSizeIterator<Item = usize>) {
        // Scattered stores cost roughly eight sequential slot writes each.
        if indices.len() * 8 >= self.slots.len() {
            self.slots.fill(None);
        } else {
            indices.for_each(|index| self.remove(index));
        }
    }

    /// Releases slots at and beyond `end`.
    pub(crate) fn shrink_to(&mut self, end: usize) {
        self.slots.truncate(end);
        self.slots.shrink_to_fit();
    }
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
