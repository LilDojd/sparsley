/// A type that can key into a sparse set.
///
/// Keys map to sparse slots through [`Key::slot`]. Sparse storage grows to
/// the largest slot inserted, so slots should be small and dense, such as
/// entity ids.
///
/// `slot` must be deterministic and injective: equal keys must produce equal
/// slots, distinct keys distinct slots. Breaking this results in an
/// unspecified behavior.
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
///     fn slot(self) -> usize {
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
    #[doc(alias = "index")]
    fn slot(self) -> usize;
}

macro_rules! impl_key {
    ($($ty:ty),* $(,)?) => {$(
        impl Key for $ty {
            #[inline]
            #[allow(clippy::cast_possible_truncation, reason = "only implemented where lossless")]
            fn slot(self) -> usize {
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
