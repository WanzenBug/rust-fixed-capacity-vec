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
    /// {
    ///     let (_, mut extend) = vec.with_fixed_capacity(4);
    ///     extend.extend_from_slice(&[5, 6, 7, 8]);
    /// }
    /// assert_eq!(&vec[..], &[1, 2, 3, 4, 5, 6, 7, 8]);
    /// ```
    #[inline]
    pub fn extend_from_slice(&mut self, other: &[T]) {
        assert!(other.len() <= self.additional_cap());
        unsafe {
            let len = self.buffer.len();
            self.buffer.set_len(len + other.len());
            self.buffer.get_unchecked_mut(len..).copy_from_slice(other);
        }
    }

    /// Extend the buffer by repeating the given slice
    ///
    /// # Panics
    ///
    /// If the slice length times the number of repetitions would overflow
    ///
    /// If the number of repetitions would exceed the capacity
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_capacity_vec::AsFixedCapacityVec;
    /// let mut vec = Vec::new();
    /// {
    ///     let (_, mut append) = vec.with_fixed_capacity(8);
    ///     append.extend_with_repeat(&[0, 1], 4);
    /// }
    /// assert_eq!(&vec[..], &[0, 1, 0, 1, 0, 1, 0, 1]);
    /// ```
    #[inline]
    pub fn extend_with_repeat(&mut self, slice: &[T], n: usize) {
        let cap_needed = slice.len().checked_mul(n).expect("capacity overflow");
        assert!(cap_needed <= self.additional_cap());

        // If `n` is larger than zero, it can be split as
        // `n = 2^expn + rem (2^expn > rem, expn >= 0, rem >= 0)`.
        // `2^expn` is the number represented by the leftmost '1' bit of `n`,
        // and `rem` is the remaining part of `n`.

        // `2^expn` repetition is done by doubling `buf` `expn`-times.
        let start_pos = self.buffer.len();
        let buf_start = unsafe { (self.buffer.as_mut_ptr() as *mut T).add(start_pos) };
        let mut buf_fill = buf_start;
        let mut copy_size = slice.len();

        // Initial copy from slice, all other copies will source from the already copied data
        unsafe {
            std::ptr::copy_nonoverlapping(slice.as_ptr(), buf_fill, copy_size);
            buf_fill = buf_fill.add(copy_size);
        }
        {
            let mut m = n >> 1;
            // If `m > 0`, there are remaining bits up to the leftmost '1'.
            while m > 0 {
                // `buf.extend(buf)`:
                unsafe {
                    std::ptr::copy_nonoverlapping(buf_start, buf_fill, copy_size);
                    buf_fill = buf_fill.add(copy_size);
                }

                copy_size <<= 1;
                m >>= 1;
            }
        }

        // `rem` (`= n - 2^expn`) repetition is done by copying
        // first `rem` repetitions from `buf` itself.
        let rem_len = cap_needed - copy_size; // `self.len() * rem`
        if rem_len > 0 {
            // `buf.extend(buf[0 .. rem_len])`:
            unsafe {
                // This is non-overlapping since `2^expn > rem`.
                std::ptr::copy_nonoverlapping(buf_start, buf_fill, rem_len);
            }
        }
        unsafe {
            self.buffer.set_len(start_pos + cap_needed);
        }
    }
}
impl<'a, T> FixedCapacityVec<'a, T>
where
    T: 'a,
{
    /// Returns the number of "empty" slots in this FixedCapacityVec
    #[inline]
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
    #[inline]
    pub fn push(&mut self, item: T) {
        assert!(self.additional_cap() > 0);
        self.buffer.push(item)
    }

    #[inline]
    pub fn capacity(&mut self) -> usize {
        self.max_len - self.start
    }

    #[inline]
    pub fn len(&mut self) -> usize {
        self.buffer.len() - self.start
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
        for orig_capacity in 1..40 {
            // 0 currently not supported
            for length in 0..40 {
                for new_capacity in 0..40 {
                    let mut vec = Vec::with_capacity(orig_capacity);
                    vec.resize(length, 0);
                    {
                        let (_, mut extend) = vec.with_fixed_capacity(new_capacity);
                        // test that this capacity can actually be filled
                        for _ in 0..new_capacity {
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

    #[test]
    fn test_extend_with_repeat_empty() {
        let mut vec: Vec<i32> = Vec::new();
        {
            let (_, mut extend) = vec.with_fixed_capacity(3);
            extend.extend_with_repeat(&[], 10);
        }
        assert_eq!(vec.len(), 0);
    }

    #[test]
    #[should_panic]
    fn test_extend_with_repeat_overflow() {
        let mut vec = Vec::new();
        let (_, mut extend) = vec.with_fixed_capacity(4);
        extend.extend_with_repeat(&[1, 2, 3, 4], usize::max_value());
    }

    #[test]
    #[should_panic]
    fn test_extend_with_repeat_capacity() {
        let mut vec = Vec::new();
        let (_, mut extend) = vec.with_fixed_capacity(1);
        extend.extend_with_repeat(&[1], 5);
    }
}
