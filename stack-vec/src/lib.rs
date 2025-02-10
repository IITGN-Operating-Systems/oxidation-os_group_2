#![no_std]

#[cfg(test)]
mod tests;

use core::ops::{Deref, DerefMut, Index, IndexMut};

/// A contiguous array type backed by a slice.
///
/// StackVec's functionality is similar to that of std::Vec. You can push
/// and pop and iterate over the vector. Unlike Vec, however, StackVec
/// requires no memory allocation as it is backed by a user-supplied slice. As a
/// result, StackVec's capacity is bounded by the user-supplied slice. This
/// results in push being fallible: if push is called when the vector is
/// full, an Err is returned.
#[derive(Debug)]
pub struct StackVec<'a, T: 'a> {
    storage: &'a mut [T],
    len: usize,
}

impl<'a, T> StackVec<'a, T> {
    /// Constructs a new, empty StackVec<T> using storage as the backing
    /// store. The returned StackVec will be able to hold storage.len() values.
    pub fn new(storage: &'a mut [T]) -> StackVec<'a, T> {
        StackVec { storage, len: 0 }
    }

    /// Constructs a new StackVec<T> using storage as the backing store.
    /// The first len elements of storage are treated as if they were `push`ed
    /// onto self. The returned StackVec will be able to hold a total of
    /// storage.len() values.
    ///
    /// # Panics
    ///
    /// Panics if len > storage.len().
    pub fn with_len(storage: &'a mut [T], len: usize) -> StackVec<'a, T> {
        assert!(
            len <= storage.len(),
            "Initial length {} exceeds storage capacity {}",
            len,
            storage.len()
        );
        StackVec { storage, len }
    }

    /// Returns the number of elements this vector can hold.
    pub fn capacity(&self) -> usize {
        self.storage.len()
    }

    /// Shortens the vector, keeping the first len elements. If len is
    /// greater than the vector's current length, this has no effect.
    /// Note that this method has no effect on the capacity of the vector.
    pub fn truncate(&mut self, len: usize) {
        if len < self.len {
            for i in len..self.len {
                unsafe {
                    core::ptr::drop_in_place(&mut self.storage[i]);
                }
            }
            self.len = len;
        }
    }

    /// Extracts a slice containing the entire vector, consuming self.
    ///
    /// Note that the returned slice's length will be the length of this vector,
    /// not the length of the original backing storage.
    pub fn into_slice(self) -> &'a mut [T] {
        &mut self.storage[..self.len]
    }

    /// Extracts a slice containing the entire vector.
    pub fn as_slice(&self) -> &[T] {
        &self.storage[..self.len]
    }

    /// Extracts a mutable slice of the entire vector.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.storage[..self.len]
    }

    /// Returns the number of elements in the vector (its 'length').
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the vector contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns true if the vector is at capacity.
    pub fn is_full(&self) -> bool {
        self.len == self.storage.len()
    }

    /// Appends value to the back of this vector if the vector is not full.
    ///
    /// # Error
    ///
    /// If this vector is full, an Err is returned. Otherwise, Ok is returned.
    pub fn push(&mut self, value: T) -> Result<(), ()> {
        if self.is_full() {
            Err(())
        } else {
            self.storage[self.len] = value;
            self.len += 1;
            Ok(())
        }
    }

    /// Returns an iterator over the elements of the vector.
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// Returns a mutable iterator over the elements of the vector.
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }
}

impl<'a, T: Clone> StackVec<'a, T> {
    /// If this vector is not empty, removes the last element from this vector
    /// by cloning it and returns it. Otherwise returns None.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            Some(self.storage[self.len].clone())
        }
    }
}

/// Allow StackVec to be used as a slice.
impl<'a, T> Deref for StackVec<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

/// Allow StackVec to be mutably used as a slice.
impl<'a, T> DerefMut for StackVec<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

/// Allow indexing into a StackVec (e.g. stack_vec[0]).
impl<'a, T> Index<usize> for StackVec<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.as_slice()[index]
    }
}

/// Allow mutable indexing into a StackVec (e.g. stack_vec[0] = ...).
impl<'a, T> IndexMut<usize> for StackVec<'a, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.as_mut_slice()[index]
    }
}

/// An iterator that consumes a StackVec, yielding elements by value.
///
/// This iterator “steals” each element out of the backing storage. Note that
/// since StackVec does not own its memory (it borrows it from the caller),
/// this implementation uses unsafe code to move out the elements.
pub struct StackVecIntoIter<'a, T> {
    vec: StackVec<'a, T>,
    index: usize,
}

impl<'a, T> Iterator for StackVecIntoIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.index < self.vec.len {
            let result = unsafe { core::ptr::read(&self.vec.storage[self.index]) };
            self.index += 1;
            Some(result)
        } else {
            None
        }
    }
}

impl<'a, T> Drop for StackVecIntoIter<'a, T> {
    fn drop(&mut self) {
        for i in self.index..self.vec.len {
            unsafe {
                core::ptr::drop_in_place(&mut self.vec.storage[i]);
            }
        }
    }
}

/// Implement IntoIterator for an owned StackVec.
// impl<'a, T> IntoIterator for StackVec<'a, T> {
//     type Item = T;
//     type IntoIter = StackVecIntoIter<'a, T>;

//     fn into_iter(self) -> Self::IntoIter {
//         StackVecIntoIter { vec: self, index: 0 }
//     }
// }

impl<'a, T> IntoIterator for StackVec<'a, T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_slice().iter()
    }
}


/// Implement IntoIterator for a shared reference to StackVec.
impl<'a, T> IntoIterator for &'a StackVec<'a, T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Implement IntoIterator for a mutable reference to StackVec.
impl<'a, T> IntoIterator for &'a mut StackVec<'a, T> {
    type Item = &'a mut T;
    type IntoIter = core::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}