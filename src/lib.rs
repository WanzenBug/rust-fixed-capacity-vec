//! Extend Vec to allow reference to content while pushing new elements.
//!
//! This is like `slice::split_at_mut` but instead of splitting into two
//! mutable slices, it splits into a slice and a Vec-like struct that can be expanded until
//! the given capacity is reached.
//!
//! # Examples
//!
//! ```
//! use fixed_capacity_vec::AsFixedCapacityVec;
//!
//! let mut vec = vec![1, 2, 3, 4];
//! {
//!     let (content, mut extend_end) = vec.with_fixed_capacity(5);
//!     extend_end.push(4);
//!     assert_eq!(extend_end.as_ref(), &[4]);
//!
//!     // We can still access content here.
//!     assert_eq!(content, &[1, 2, 3, 4]);
//!
//!     // We can even copy one buffer into the other
//!     extend_end.extend_from_slice(content);
//!     assert_eq!(extend_end.as_ref(), &[4, 1, 2, 3, 4]);
//!
//!     // The following line would panic because we reached max. capacity:
//!     // extend_end.push(10);
//! }
//! // All operations happened on vec
//! assert_eq!(vec, &[1, 2, 3, 4, 4, 1, 2, 3, 4]);
//! ```

use std::convert::AsMut;
use std::convert::AsRef;
use std::ops::Deref;
use std::ops::DerefMut;
use std::slice;

/// Allows pushing to a Vec while keeping a reference to it's content.
pub trait AsFixedCapacityVec {
    type Item;

    /// Split a vec to create an initialized "read" view and an extendable "write" view
    ///
    /// Allow extending a Vec while keeping a reference to the previous content. The "read" view
    /// is a a mutable slice, while "write" behaves like a Vec with `capacity`. Other than a normal
    /// Vec, "write" panics if it would need to reallocate.
    ///
    /// # Panics
    ///
    /// Panics if `pos` > `self.len()`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    /// ```
    /// use fixed_capacity_vec::AsFixedCapacityVec;
    /// let mut vec = Vec::new();
    /// vec.push(1);
    /// vec.push(2);
    /// {
    ///     let (old_data, mut extent) = vec.with_fixed_capacity(4);
    ///     extent.extend_from_slice(old_data);
    ///     extent.extend_from_slice(old_data);
    /// }
    /// assert_eq!(vec, &[1, 2, 1, 2, 1, 2]);
    /// ```
    fn with_fixed_capacity(
        &mut self,
        capacity: usize,
    ) -> (&mut [Self::Item], FixedCapacityVec<Self::Item>);
}

/// A safe wrapper around a Vec which is not allowed to reallocate
#[derive(Debug)]
pub struct FixedCapacityVec<'a, T>
where
    T: 'a,
{
    start: usize,
    max_len: usize,
    buffer: &'a mut Vec<T>,
}

impl<T> AsFixedCapacityVec for Vec<T> {
    type Item = T;

    fn with_fixed_capacity(&mut self, capacity: usize) -> (&mut [T], FixedCapacityVec<T>) {
        let len = self.len();
        // Ensure the vector can fit `capacity` more elements after its current len() without reallocating
        self.reserve(capacity);
        debug_assert!(self.capacity() - len >= capacity);

        // Vec's internal pointer should always point to a non-null pointer. This is important for
        // slice's from_raw_parts method.
        // TODO: Check if this assert is needed
        assert!(self.capacity() > 0);
        let raw_ptr = self.as_mut_ptr();
        let init_slice = unsafe { slice::from_raw_parts_mut(raw_ptr, len) };

        (
            init_slice,
            FixedCapacityVec {
                start: len,
                max_len: len + capacity,
                buffer: self,
            },
        )
    }
}

impl<'a, T> FixedCapacityVec<'a, T>
where
    T: 'a + Copy,
{
    /// Appends all elements in a slice to the buffer.
    ///
    /// # Panics
    ///
    /// If the FixedCapacityVec does not have enough capacity left to extend all
    /// elements
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_capacity_vec::AsFixedCapacityVec;
    /// let mut vec = vec![1, 2, 3, 4];
    /// vec.reserve(4);
    /// {
    ///     let (_, mut extend) = vec.with_fixed_capacity(4);
    ///     extend.extend_from_slice(&[5, 6, 7, 8]);
    /// }
    /// assert_eq!(&vec[..], &[1, 2, 3, 4, 5, 6, 7, 8]);
    /// ```
    pub fn extend_from_slice(&mut self, other: &[T]) {
        assert!(other.len() <= self.additional_cap());
        unsafe {
            let len = self.buffer.len();
            self.buffer.set_len(len + other.len());
            self.buffer.get_unchecked_mut(len..).copy_from_slice(other);
        }
    }
}
impl<'a, T> FixedCapacityVec<'a, T>
where
    T: 'a,
{
    /// Returns the number of "empty" slots in this FixedCapacityVec
    fn additional_cap(&self) -> usize {
        self.max_len - self.buffer.len()
    }

    /// Appends an element to the back of a collection.
    ///
    /// # Panics
    ///
    /// If the FixedCapacityVec is already at max. capacity
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_capacity_vec::AsFixedCapacityVec;
    /// let mut vec = vec![1, 2, 3, 4];
    /// vec.reserve(4);
    /// {
    ///     let (_, mut extend) = vec.with_fixed_capacity(4);
    ///     extend.push(5);
    ///     extend.push(6);
    /// }
    /// assert_eq!(&vec[..], &[1, 2, 3, 4, 5, 6]);
    /// ```
    pub fn push(&mut self, item: T) {
        assert!(self.additional_cap() > 0);
        self.buffer.push(item)
    }
}

impl<'a, T> Deref for FixedCapacityVec<'a, T>
where
    T: 'a,
{
    type Target = [T];

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.buffer[self.start..self.buffer.len()]
    }
}

impl<'a, T> DerefMut for FixedCapacityVec<'a, T>
where
    T: 'a,
{
    fn deref_mut(&mut self) -> &mut <Self as Deref>::Target {
        let start = self.start;
        &mut self.buffer[start..]
    }
}

impl<'a, T> Extend<T> for FixedCapacityVec<'a, T>
where
    T: 'a + Clone,
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            assert!(self.additional_cap() > 0);
            self.buffer.push(item)
        }
    }
}

impl<'a, T> AsRef<[T]> for FixedCapacityVec<'a, T>
where
    T: 'a,
{
    fn as_ref(&self) -> &[T] {
        &self[..]
    }
}

impl<'a, T> AsMut<[T]> for FixedCapacityVec<'a, T>
where
    T: 'a,
{
    fn as_mut(&mut self) -> &mut [T] {
        &mut self[..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_permutations_in_constructor() {
        for orig_capacity in 1..40 { // 0 currently not supported
            for length in 0..40 {
                for new_capacity in 0..40 {
                    let mut vec = Vec::with_capacity(orig_capacity);
                    vec.resize(length, 0);
                    {
                        let (_, mut extend) = vec.with_fixed_capacity(new_capacity);
                        // test that this capacity can actually be filled
                        for i in 0..new_capacity {
                            extend.push(0);
                        }
                    }
                    assert_eq!(vec.len(), length + new_capacity);
                    assert!(vec.capacity() >= length + new_capacity);
                }
            }
        }
    }

    #[test]
    fn test_extend() {
        let mut vec = Vec::new();
        {
            use std::iter::repeat;
            let (_, mut buffer) = vec.with_fixed_capacity(4);
            buffer.extend(repeat(9).take(3));
        }
        assert_eq!(&vec[..], &[9, 9, 9]);
    }

    #[test]
    #[should_panic]
    fn test_over_capacity() {
        let mut vec = Vec::new();
        vec.push(1);
        let (_, mut extend) = vec.with_fixed_capacity(1);
        extend.push(0);
        extend.push(1);
    }

    #[test]
    #[should_panic]
    fn test_empty_cap_panics() {
        let mut vec: Vec<i32> = Vec::new();
        let (_, _) = vec.with_fixed_capacity(0);
    }

    #[test]
    #[should_panic]
    fn test_slice_over_cap() {
        let mut vec = Vec::new();
        let (_, mut extend) = vec.with_fixed_capacity(2);
        extend.extend_from_slice(&[1, 2, 3]);
    }

    #[test]
    #[should_panic]
    fn test_iter_over_cap() {
        let mut vec = Vec::new();
        let (_, mut extend) = vec.with_fixed_capacity(2);
        extend.extend(::std::iter::repeat(2).take(3));
    }
}
