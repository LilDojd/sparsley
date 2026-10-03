//! A bounded integer map with tightly packed, contiguous values.
//!
//! Sparse indices are never eagerly initialized. Epoch-stamped occupancy bits
//! prove which indices may be read, without a dependent dense-key lookup.
//! Keyed operations panic outside the explicitly allocated `0..capacity()` range.
//! Dense storage grows independently; iteration order and dense indices may
//! change after removal. Keys are plain integers, **not generational entities**.
//!
//! ```
//! use sparsley::SparseSet;
//!
//! let mut components = SparseSet::with_capacity(1024);
//! assert_eq!(components.insert(7, 10), None);
//! assert_eq!(components.insert(7, 20), Some(10));
//! components.insert(900, 30);
//! for (key, value) in &mut components {
//!     *value += key as i32;
//! }
//! assert_eq!(components.get(7), Some(&27));
//! assert_eq!(components.values().len(), 2); // A real contiguous &[T].
//! components.retain(|_, value| *value < 100);
//! assert_eq!(components.remove(7), Some(27));
//! ```

#![deny(unsafe_op_in_unsafe_fn)]

use std::fmt;
use std::iter::{Copied, Zip};
use std::mem::{self, MaybeUninit};
use std::num::NonZeroUsize;
use std::{slice, vec};

const WORD_BITS: usize = usize::BITS as usize;

/// Membership for one native-word-sized block of keys.
/// A stale epoch makes every bit inactive, without clearing the stored indices.
#[derive(Clone, Copy, Default)]
struct OccupancyWord {
    bits: usize,
    // None means never used/reset; Option uses zero's niche with no extra bytes.
    epoch: Option<NonZeroUsize>,
}

impl OccupancyWord {
    #[inline]
    fn contains(&self, key: usize, epoch: NonZeroUsize) -> bool {
        // Both fields are initialized, including in stale words. Boolean &
        // avoids short-circuiting; it never reads an uninitialized sparse slot.
        self.is_current(epoch) & (self.bits & Self::mask(key) != 0)
    }

    #[inline]
    fn insert(&mut self, key: usize, epoch: NonZeroUsize) {
        if !self.is_current(epoch) {
            self.bits = 0;
            self.epoch = Some(epoch);
        }
        self.bits |= Self::mask(key);
    }

    #[inline]
    fn remove(&mut self, key: usize) {
        self.bits &= !Self::mask(key);
    }

    #[inline]
    fn is_current(&self, epoch: NonZeroUsize) -> bool {
        // None maps to zero, which cannot equal the current epoch. Comparing the
        // numeric encoding avoids a redundant Option discriminant check.
        self.epoch.map_or(0, NonZeroUsize::get) == epoch.get()
    }

    #[inline]
    fn mask(key: usize) -> usize {
        1usize << (key % WORD_BITS)
    }
}

/// Shared iteration over `(key, &value)` pairs in unspecified dense order.
pub type Iter<'a, T> = Zip<Copied<slice::Iter<'a, usize>>, slice::Iter<'a, T>>;
/// Mutable iteration over `(key, &mut value)` pairs; keys cannot be changed.
pub type IterMut<'a, T> = Zip<Copied<slice::Iter<'a, usize>>, slice::IterMut<'a, T>>;
/// Owned iteration over `(key, value)` pairs.
pub type IntoIter<T> = Zip<vec::IntoIter<usize>, vec::IntoIter<T>>;
/// Draining iteration, retaining allocations and dropping unconsumed values.
pub type Drain<'a, T> = Zip<vec::Drain<'a, usize>, vec::Drain<'a, T>>;

/// An O(1)-lookup integer map with O(len) packed iteration.
///
/// Use this when keys have a reasonably small upper bound. Sparse storage costs
/// one `usize` per allowed key plus 1/4 byte per key (rounded to whole bitmap
/// words). Dense storage costs one key and one value per reserved entry, without
/// key/value padding. Very large, scattered keys are better suited to a hash map
/// or paged ECS storage.
///
/// # Internal invariants
///
/// * `keys.len() == values.len()`; `keys` contains distinct, in-range keys.
/// * A bit is live iff its word's epoch equals `Some(epoch)` and the key is present.
/// * For every live key, its sparse slot is initialized and points at the matching
///   position in **both** dense vectors. Dead sparse slots are never read.
/// * Public mutable access exposes only values, never structural keys.
///
/// Every structural operation restores these invariants before running user
/// code (predicates or destructors). Allocation precedes changes to lengths.
pub struct SparseSet<T> {
    keys: Vec<usize>,
    values: Vec<T>,
    sparse: Box<[MaybeUninit<usize>]>,
    occupancy: Box<[OccupancyWord]>,
    // The current epoch is always nonzero; rollover invalidates all words.
    epoch: NonZeroUsize,
}

impl<T> SparseSet<T> {
    /// Allocates the key range `0..capacity`, without reserving dense entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacities(capacity, 0)
    }

    /// Allocates the key range and reserves space for `dense_capacity` entries.
    ///
    /// # Panics
    /// Panics if the dense reservation exceeds the number of allowed keys, or
    /// any allocation exceeds the allocator's size limit.
    pub fn with_capacities(capacity: usize, dense_capacity: usize) -> Self {
        assert!(
            dense_capacity <= capacity,
            "dense capacity exceeds key space"
        );
        Self {
            keys: Vec::with_capacity(dense_capacity),
            values: Vec::with_capacity(dense_capacity),
            sparse: Box::<[usize]>::new_uninit_slice(capacity),
            occupancy: vec![OccupancyWord::default(); capacity.div_ceil(WORD_BITS)]
                .into_boxed_slice(),
            epoch: NonZeroUsize::MIN,
        }
    }

    /// Number of live entries.
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether there are no live entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The exclusive key-space bound, **not** the dense reservation.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.sparse.len()
    }

    /// Number of entries that can fit without reallocating either dense vector.
    pub fn dense_capacity(&self) -> usize {
        self.keys.capacity().min(self.values.capacity())
    }

    /// Reserves room for at least `additional` more dense entries.
    ///
    /// Does not change the allowed key range. Allocation failure leaves all live
    /// entries intact, although one vector's reservation may already have grown.
    ///
    /// # Panics
    /// Panics if the reservation exceeds the allocator's size limit.
    pub fn reserve(&mut self, additional: usize) {
        self.keys.reserve(additional);
        self.values.reserve(additional);
    }

    /// Releases unused dense reservation, without changing the key range.
    pub fn shrink_to_fit(&mut self) {
        self.keys.shrink_to_fit();
        self.values.shrink_to_fit();
    }

    /// Explicitly grows the allowed key range, preserving live entries.
    ///
    /// Smaller or equal requests do nothing. O(old capacity) bytes are copied;
    /// copying `MaybeUninit` does **not** read its contents as initialized integers.
    ///
    /// # Panics
    /// Panics if the allocation exceeds the allocator's size limit.
    pub fn grow_key_capacity(&mut self, capacity: usize) {
        if capacity <= self.capacity() {
            return;
        }
        let mut sparse = Box::<[usize]>::new_uninit_slice(capacity);
        sparse[..self.capacity()].copy_from_slice(&self.sparse);
        let mut occupancy =
            vec![OccupancyWord::default(); capacity.div_ceil(WORD_BITS)].into_boxed_slice();
        occupancy[..self.occupancy.len()].copy_from_slice(&self.occupancy);
        // Commit only after both allocations succeed.
        self.sparse = sparse;
        self.occupancy = occupancy;
    }

    /// Inserts a value, returning ownership of the previous value if replaced.
    ///
    /// # Panics
    /// Panics if `key >= capacity()` or dense allocation overflows.
    #[inline]
    pub fn insert(&mut self, key: usize, value: T) -> Option<T> {
        if let Some(index) = self.dense_index(key) {
            // SAFETY: dense_index proves a live index in both vectors. The
            // exclusive borrow prevents simultaneous access to this value.
            let stored = unsafe { self.values.get_unchecked_mut(index) };
            return Some(mem::replace(stored, value));
        }
        let index = self.len();
        // Reserve both arrays before writing or extending either live prefix.
        if index == self.dense_capacity() {
            self.grow_dense();
        }
        // SAFETY: index equals both lengths; reserve above guarantees space in
        // both allocations. These writes cannot panic or run user code. Update
        // lengths only after both slots are initialized, without redundant push
        // capacity checks. For ZSTs the aligned Vec pointer permits a zero-byte
        // write; the non-ZST key vector bounds index + 1 against overflow.
        unsafe {
            self.keys.as_mut_ptr().add(index).write(key);
            self.values.as_mut_ptr().add(index).write(value);
            self.keys.set_len(index + 1);
            self.values.set_len(index + 1);
        }
        // SAFETY: dense_index already checked key < capacity. Growing dense
        // storage never changes sparse/occupancy lengths, which cover every key.
        // Publish occupancy only AFTER the slot and both dense entries exist.
        unsafe {
            self.sparse.get_unchecked_mut(key).write(index);
            self.occupancy
                .get_unchecked_mut(key / WORD_BITS)
                .insert(key, self.epoch);
        }
        None
    }

    // Reserve space for one more entry, not an exact +1 capacity increase:
    // Vec::reserve uses amortized growth and may allocate extra capacity.
    // Keep allocation machinery out of the hot insertion path, as Vec does.
    #[cold]
    #[inline(never)]
    fn grow_dense(&mut self) {
        self.reserve(1);
    }

    /// Tests membership without reading sparse indices or dense storage.
    ///
    /// # Panics
    /// Panics if `key >= capacity()`.
    #[inline]
    pub fn contains(&self, key: usize) -> bool {
        assert!(key < self.capacity(), "key outside sparse-set capacity");
        // SAFETY: occupancy.len() == capacity.div_ceil(WORD_BITS), and key is in range.
        let word = unsafe { self.occupancy.get_unchecked(key / WORD_BITS) };
        word.contains(key, self.epoch)
    }

    /// Returns a live key's dense position, valid until the next structural change.
    ///
    /// # Panics
    /// Panics if `key >= capacity()`.
    #[inline]
    pub fn dense_index(&self, key: usize) -> Option<usize> {
        if !self.contains(key) {
            return None;
        }
        // SAFETY: contains checks the key bound. A current-epoch live bit proves
        // this slot was written and its index is valid in both dense vectors.
        Some(unsafe { self.sparse.get_unchecked(key).assume_init_read() })
    }

    /// Gets a value; returns None for an absent in-range key.
    ///
    /// # Panics
    /// Panics if `key >= capacity()`.
    #[inline]
    pub fn get(&self, key: usize) -> Option<&T> {
        let index = self.dense_index(key)?;
        // SAFETY: dense_index proves index < values.len(); borrow is shared.
        Some(unsafe { self.values.get_unchecked(index) })
    }

    /// Mutably gets a value, without exposing the structural key.
    ///
    /// # Panics
    /// Panics if `key >= capacity()`.
    #[inline]
    pub fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        let index = self.dense_index(key)?;
        // SAFETY: dense_index proves index < values.len(); borrow is exclusive.
        Some(unsafe { self.values.get_unchecked_mut(index) })
    }

    /// Removes and returns a value, repairing a swapped-in entry's index.
    ///
    /// # Panics
    /// Panics if `key >= capacity()`.
    #[inline]
    pub fn remove(&mut self, key: usize) -> Option<T> {
        let index = self.dense_index(key)?;
        // SAFETY: dense_index proves key maps to index in both dense vectors.
        Some(unsafe { self.remove_at_unchecked(key, index) })
    }

    /// # Safety
    /// `index` must be live and `keys[index]` must equal `key`.
    unsafe fn remove_at_unchecked(&mut self, key: usize, index: usize) -> T {
        let last = self.len() - 1;
        // SAFETY: the caller proves index < len, hence len > 0 and last is also
        // valid. keys and values have identical lengths. Read the removed value
        // once, move the tail into its place, then exclude the old tail from Drop.
        // copy (not copy_nonoverlapping) permits removing the last entry itself.
        // The caller supplies the known key, avoiding a dependent dense-key load
        // before clearing its occupancy bit.
        let (moved_key, value) = unsafe {
            let moved_key = *self.keys.get_unchecked(last);
            let values = self.values.as_mut_ptr();
            let value = values.add(index).read();
            std::ptr::copy(values.add(last), values.add(index), 1);
            *self.keys.get_unchecked_mut(index) = moved_key;
            self.keys.set_len(last);
            self.values.set_len(last);
            (moved_key, value)
        };
        // SAFETY: both keys came from immutable, in-range structural keys.
        // A removed-tail repair may leave a dead slot pointing at len; its bit
        // is cleared first, so that slot can never be read until rewritten.
        unsafe {
            self.occupancy
                .get_unchecked_mut(key / WORD_BITS)
                .remove(key);
            self.sparse.get_unchecked_mut(moved_key).write(index);
        }
        value
    }

    /// Clears all entries, retaining allocations.
    ///
    /// O(len) when values need dropping; otherwise O(1), except once per
    /// `usize::MAX` clears when epoch rollover resets O(capacity / WORD_BITS)
    /// metadata. Membership is invalidated before destructors run, so unwinding
    /// cannot expose old indices.
    pub fn clear(&mut self) {
        self.invalidate();
        self.keys.clear();
        self.values.clear();
    }

    fn invalidate(&mut self) {
        if let Some(epoch) = self.epoch.checked_add(1) {
            self.epoch = epoch;
        } else {
            self.occupancy.fill(OccupancyWord::default());
            self.epoch = NonZeroUsize::MIN;
        }
    }

    /// Removes all entries, yielding their owned pairs and retaining allocations.
    ///
    /// Dropping the iterator drops remaining values. Forgetting it may leak those
    /// values, but still leaves the set empty and safe to reuse.
    pub fn drain(&mut self) -> Drain<'_, T> {
        self.invalidate();
        self.keys.drain(..).zip(self.values.drain(..))
    }

    /// Retains entries for which the predicate returns true, allowing value edits.
    ///
    /// Each entry is visited once, in reverse dense order. If the predicate or a
    /// removed value's destructor panics, previously finished removals stay
    /// finished and every remaining entry is still reachable. O(len).
    pub fn retain(&mut self, mut keep: impl FnMut(usize, &mut T) -> bool) {
        for index in (0..self.len()).rev() {
            let key = self.keys[index];
            if !keep(key, &mut self.values[index]) {
                // SAFETY: walking backwards leaves len >= index + 1, regardless
                // of prior removals. Predicates cannot change structural keys.
                drop(unsafe { self.remove_at_unchecked(key, index) });
            }
        }
    }

    /// Immutable structural keys, aligned with [`Self::values`].
    ///
    /// ```compile_fail
    /// use sparsley::SparseSet;
    /// let mut set = SparseSet::<u64>::with_capacity(2);
    /// set.insert(0, 10);
    /// set.insert(1, 20);
    /// set.keys().swap(0, 1); // Structural keys cannot be reordered.
    /// ```
    pub fn keys(&self) -> &[usize] {
        &self.keys
    }

    /// Contiguous values, suitable for vectorized CPU or GPU-buffer processing.
    pub fn values(&self) -> &[T] {
        &self.values
    }

    /// Contiguous mutable values. Reordering values changes their key association,
    /// but cannot corrupt the sparse map because structural keys stay immutable.
    pub fn values_mut(&mut self) -> &mut [T] {
        &mut self.values
    }

    /// Aligned key/value slices, with keys immutable and values mutable.
    pub fn as_mut_slices(&mut self) -> (&[usize], &mut [T]) {
        (&self.keys, &mut self.values)
    }

    /// Iterates `(key, &value)` pairs in unspecified dense order.
    pub fn iter(&self) -> Iter<'_, T> {
        self.keys.iter().copied().zip(self.values.iter())
    }

    /// Iterates `(key, &mut value)` pairs without exposing mutable keys.
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self.keys.iter().copied().zip(self.values.iter_mut())
    }
}

impl<T: fmt::Debug> fmt::Debug for SparseSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<T> IntoIterator for SparseSet<T> {
    type Item = (usize, T);
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.keys.into_iter().zip(self.values)
    }
}

impl<'a, T> IntoIterator for &'a SparseSet<T> {
    type Item = (usize, &'a T);
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut SparseSet<T> {
    type Item = (usize, &'a mut T);
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
