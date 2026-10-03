/// A type that can key into a sparse set.
///
/// Keys map to sparse slots through [`Key::index`]. Sparse storage grows to
/// the largest index inserted, so indices should be small and dense, such as
/// entity ids.
///
/// `index` must be deterministic and injective: equal keys must produce equal
/// indices, distinct keys distinct indices. Breaking this is a logic error.
/// The behavior is then unspecified (wrong results or panics) but never
/// undefined.
///
/// # Examples
///
/// ```
/// use sparsley::{Key, SparseMap};
///
/// #[derive(Clone, Copy)]
/// struct Entity(u32);
///
/// impl Key for Entity {
///     fn index(self) -> usize {
///         self.0 as usize
///     }
/// }
///
/// let mut names = SparseMap::new();
/// names.insert(Entity(4), "four");
/// assert_eq!(names.get(Entity(4)), Some(&"four"));
/// ```
pub trait Key: Copy {
    /// Returns the sparse slot of this key.
    fn index(self) -> usize;
}

macro_rules! impl_key {
    ($($ty:ty),* $(,)?) => {$(
        impl Key for $ty {
            #[inline]
            #[allow(clippy::cast_possible_truncation, reason = "only implemented where lossless")]
            fn index(self) -> usize {
                self as usize
            }
        }
    )*};
}

impl_key!(u8, u16, usize);

#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
impl_key!(u32);

#[cfg(target_pointer_width = "64")]
impl_key!(u64);
