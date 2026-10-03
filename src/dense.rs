use alloc::alloc::{Layout, alloc, dealloc, handle_alloc_error};
use core::marker::PhantomData;
use core::mem;
use core::ptr::{self, NonNull};
use core::slice;

/// Largest number of entries; positions fit in a `u32` slot as `position + 1`.
pub(crate) const MAX_LEN: usize = u32::MAX as usize;

/// Parallel key and value arrays sharing one allocation, length and capacity.
///
/// # Invariants
///
/// * `len <= cap <= MAX_LEN`.
/// * `keys[..len]` and `values[..len]` are initialized.
/// * If `layout(cap)` is non-empty, `keys` points to an allocation of that
///   layout and `values` to its value array; otherwise both are dangling.
pub(crate) struct Dense<K, V> {
    keys: NonNull<K>,
    values: NonNull<V>,
    len: usize,
    cap: usize,
    marker: PhantomData<(K, V)>,
}

// SAFETY: `Dense` owns its keys and values like `Vec` does.
unsafe impl<K: Send, V: Send> Send for Dense<K, V> {}
// SAFETY: shared access only hands out shared references.
unsafe impl<K: Sync, V: Sync> Sync for Dense<K, V> {}

impl<K, V> Dense<K, V> {
    const IS_ZST: bool = mem::size_of::<K>() == 0 && mem::size_of::<V>() == 0;

    pub(crate) const fn new() -> Self {
        Self {
            keys: NonNull::dangling(),
            values: NonNull::dangling(),
            len: 0,
            cap: if Self::IS_ZST { MAX_LEN } else { 0 },
            marker: PhantomData,
        }
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        let mut dense = Self::new();
        dense.reserve(capacity);
        dense
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        self.cap
    }

    #[inline]
    pub(crate) fn keys(&self) -> &[K] {
        // SAFETY: `keys[..len]` is initialized and borrowed through `self`.
        unsafe { slice::from_raw_parts(self.keys.as_ptr(), self.len) }
    }

    #[inline]
    pub(crate) fn values(&self) -> &[V] {
        // SAFETY: `values[..len]` is initialized and borrowed through `self`.
        unsafe { slice::from_raw_parts(self.values.as_ptr(), self.len) }
    }

    #[inline]
    pub(crate) fn values_mut(&mut self) -> &mut [V] {
        // SAFETY: `values[..len]` is initialized and exclusively borrowed.
        unsafe { slice::from_raw_parts_mut(self.values.as_ptr(), self.len) }
    }

    #[inline]
    pub(crate) fn slices_mut(&mut self) -> (&[K], &mut [V]) {
        // SAFETY: both prefixes are initialized and the arrays are disjoint.
        unsafe {
            (
                slice::from_raw_parts(self.keys.as_ptr(), self.len),
                slice::from_raw_parts_mut(self.values.as_ptr(), self.len),
            )
        }
    }

    #[inline]
    pub(crate) fn is_full(&self) -> bool {
        self.len == self.cap
    }

    /// Appends an entry, returning its value.
    #[inline]
    pub(crate) fn push(&mut self, key: K, value: V) -> &mut V {
        if self.is_full() {
            self.grow_one();
        }
        // SAFETY: `grow_one` made room.
        unsafe { self.push_unchecked(key, value) }
    }

    /// Appends an entry without checking capacity, returning its value.
    ///
    /// # Safety
    ///
    /// The storage must not be full.
    #[inline]
    pub(crate) unsafe fn push_unchecked(&mut self, key: K, value: V) -> &mut V {
        let position = self.len;
        debug_assert!(position < self.cap);
        // SAFETY: `position < cap`, so both slots are allocated and unused.
        unsafe {
            self.keys.as_ptr().add(position).write(key);
            let slot = self.values.as_ptr().add(position);
            slot.write(value);
            self.len = position + 1;
            &mut *slot
        }
    }

    /// Swaps the entries at `a` and `b`.
    ///
    /// # Panics
    ///
    /// Panics if either position is out of bounds.
    #[track_caller]
    pub(crate) fn swap(&mut self, a: usize, b: usize) {
        assert!(a < self.len && b < self.len, "position out of bounds");
        // SAFETY: both positions are initialized; `ptr::swap` permits `a == b`.
        unsafe {
            ptr::swap(self.keys.as_ptr().add(a), self.keys.as_ptr().add(b));
            ptr::swap(self.values.as_ptr().add(a), self.values.as_ptr().add(b));
        }
    }

    /// Removes the entry at `position`, moving the last entry into its place.
    ///
    /// # Panics
    ///
    /// Panics if `position >= len`.
    #[inline]
    pub(crate) fn swap_remove(&mut self, position: usize) -> (K, V) {
        assert!(position < self.len, "position out of bounds");
        let last = self.len - 1;
        self.len = last;
        // SAFETY: `position` and `last` are initialized. The entry at
        // `position` is read out once, then overwritten by the last entry,
        // which falls outside the new length. `ptr::copy` permits equality.
        unsafe {
            let keys = self.keys.as_ptr();
            let values = self.values.as_ptr();
            let entry = (keys.add(position).read(), values.add(position).read());
            ptr::copy(keys.add(last), keys.add(position), 1);
            ptr::copy(values.add(last), values.add(position), 1);
            entry
        }
    }

    pub(crate) fn clear(&mut self) {
        let len = mem::replace(&mut self.len, 0);
        // SAFETY: the prefixes were initialized and are no longer reachable.
        unsafe { self.drop_range(0, len) };
    }

    /// Sets the length to zero and returns the entries for reading out.
    pub(crate) fn take_entries(&mut self) -> RawEntries<K, V> {
        let end = mem::replace(&mut self.len, 0);
        RawEntries {
            keys: self.keys,
            values: self.values,
            start: 0,
            end,
        }
    }

    /// # Safety
    ///
    /// `start..end` must be initialized and never read again.
    unsafe fn drop_range(&mut self, start: usize, end: usize) {
        // SAFETY: guaranteed by the caller.
        unsafe { drop_entries(self.keys, self.values, start, end) };
    }

    /// Ensures room for `additional` more entries.
    ///
    /// # Panics
    ///
    /// Panics if the capacity would exceed `u32::MAX` entries or
    /// `isize::MAX` bytes.
    pub(crate) fn reserve(&mut self, additional: usize) {
        if additional > self.cap - self.len {
            let required = self
                .len
                .checked_add(additional)
                .filter(|&required| required <= MAX_LEN)
                .unwrap_or_else(|| capacity_overflow());
            self.reallocate(required.max(self.cap.saturating_mul(2)).min(MAX_LEN));
        }
    }

    pub(crate) fn shrink_to_fit(&mut self) {
        if !Self::IS_ZST && self.cap > self.len {
            self.reallocate(self.len);
        }
    }

    #[cold]
    #[inline(never)]
    fn grow_one(&mut self) {
        if self.cap == MAX_LEN {
            capacity_overflow();
        }
        self.reallocate((self.cap * 2).clamp(4, MAX_LEN));
    }

    /// Moves the entries into an allocation for exactly `cap` entries.
    fn reallocate(&mut self, cap: usize) {
        debug_assert!(self.len <= cap && cap <= MAX_LEN);
        let (layout, offset) = layout::<K, V>(cap).unwrap_or_else(|| capacity_overflow());
        let (keys, values) = if layout.size() == 0 {
            (NonNull::dangling(), NonNull::dangling())
        } else {
            // SAFETY: the layout has a non-zero size.
            let base = NonNull::new(unsafe { alloc(layout) })
                .unwrap_or_else(|| handle_alloc_error(layout));
            // SAFETY: `offset` is the in-bounds, aligned start of the values.
            (base.cast(), unsafe { base.byte_add(offset) }.cast())
        };
        // SAFETY: the old and new arrays are disjoint, and both hold `len`.
        unsafe {
            ptr::copy_nonoverlapping(self.keys.as_ptr(), keys.as_ptr(), self.len);
            ptr::copy_nonoverlapping(self.values.as_ptr(), values.as_ptr(), self.len);
            self.deallocate();
        }
        self.keys = keys;
        self.values = values;
        self.cap = cap;
    }

    /// # Safety
    ///
    /// The allocation must not be used afterwards.
    unsafe fn deallocate(&mut self) {
        let (layout, _) = layout::<K, V>(self.cap).expect("layout was valid at allocation");
        if layout.size() != 0 {
            // SAFETY: `keys` points to an allocation of this layout.
            unsafe { dealloc(self.keys.as_ptr().cast(), layout) };
        }
    }
}

impl<K, V> Drop for Dense<K, V> {
    fn drop(&mut self) {
        struct Deallocate<'a, K, V>(&'a mut Dense<K, V>);

        impl<K, V> Drop for Deallocate<'_, K, V> {
            fn drop(&mut self) {
                // SAFETY: the dense storage is being dropped.
                unsafe { self.0.deallocate() };
            }
        }

        let guard = Deallocate(self);
        guard.0.clear();
    }
}

impl<K: Clone, V: Clone> Clone for Dense<K, V> {
    fn clone(&self) -> Self {
        let mut dense = Self::with_capacity(self.len);
        for (key, value) in self.keys().iter().zip(self.values()) {
            dense.push(key.clone(), value.clone());
        }
        dense
    }
}

/// Initialized entries detached from a [`Dense`] for reading out by value.
pub(crate) struct RawEntries<K, V> {
    keys: NonNull<K>,
    values: NonNull<V>,
    start: usize,
    end: usize,
}

// SAFETY: `RawEntries` owns the entries it yields.
unsafe impl<K: Send, V: Send> Send for RawEntries<K, V> {}
// SAFETY: shared access exposes nothing.
unsafe impl<K: Sync, V: Sync> Sync for RawEntries<K, V> {}

impl<K, V> RawEntries<K, V> {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.end - self.start
    }

    /// # Safety
    ///
    /// The entries must still be allocated.
    #[inline]
    pub(crate) unsafe fn next(&mut self) -> Option<(K, V)> {
        if self.start == self.end {
            return None;
        }
        let position = self.start;
        self.start += 1;
        // SAFETY: `position` is initialized and has not been read.
        Some(unsafe { self.read(position) })
    }

    /// # Safety
    ///
    /// The entries must still be allocated.
    #[inline]
    pub(crate) unsafe fn next_back(&mut self) -> Option<(K, V)> {
        if self.start == self.end {
            return None;
        }
        self.end -= 1;
        // SAFETY: `end` is initialized and has not been read.
        Some(unsafe { self.read(self.end) })
    }

    unsafe fn read(&self, position: usize) -> (K, V) {
        // SAFETY: guaranteed by the caller.
        unsafe {
            (
                self.keys.as_ptr().add(position).read(),
                self.values.as_ptr().add(position).read(),
            )
        }
    }

    /// Drops the unread entries.
    ///
    /// # Safety
    ///
    /// The entries must still be allocated and are not read afterwards.
    pub(crate) unsafe fn drop_remaining(&mut self) {
        let (start, end) = (self.start, mem::replace(&mut self.end, self.start));
        // SAFETY: guaranteed by the caller.
        unsafe { drop_entries(self.keys, self.values, start, end) };
    }
}

/// Drops `keys[start..end]` and `values[start..end]`, values even if a key
/// destructor panics.
///
/// # Safety
///
/// Both ranges must be initialized and never used again.
unsafe fn drop_entries<K, V>(keys: NonNull<K>, values: NonNull<V>, start: usize, end: usize) {
    let len = end - start;
    // SAFETY: guaranteed by the caller.
    unsafe {
        let _values = DropSlice(ptr::slice_from_raw_parts_mut(
            values.as_ptr().add(start),
            len,
        ));
        ptr::drop_in_place(ptr::slice_from_raw_parts_mut(keys.as_ptr().add(start), len));
    }
}

/// Drops its slice when it goes out of scope, including during unwinding.
struct DropSlice<T>(*mut [T]);

impl<T> Drop for DropSlice<T> {
    fn drop(&mut self) {
        // SAFETY: only constructed by `drop_entries`, whose caller guarantees
        // the slice is initialized and unused.
        unsafe { ptr::drop_in_place(self.0) };
    }
}

/// The allocation layout for `cap` entries and the byte offset of the values.
fn layout<K, V>(cap: usize) -> Option<(Layout, usize)> {
    let keys = Layout::array::<K>(cap).ok()?;
    let values = Layout::array::<V>(cap).ok()?;
    keys.extend(values).ok()
}

#[cold]
#[track_caller]
fn capacity_overflow() -> ! {
    panic!("capacity overflow");
}
