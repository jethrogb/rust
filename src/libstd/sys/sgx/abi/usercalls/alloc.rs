// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unused)]

use ptr;
use mem;
use cell::UnsafeCell;
use slice;
use ops::{Deref, DerefMut, Index, IndexMut, CoerceUnsized};
use slice::SliceIndex;

use fortanix_sgx_abi::*;
use super::super::mem::is_user_range;

/// A type that can be safely read from userspace. Most types that don't have a
/// destructor and don't contain enums or references or Rust-style owned
/// pointers (Vec, Arc, etc.) should be ok. Also they shouldn't contain any 
/// internal padding.
pub unsafe trait UserSafeSized: Sized {}

unsafe impl UserSafeSized for u8 {}
unsafe impl<T> UserSafeSized for FifoDescriptor<T> {}
unsafe impl UserSafeSized for ByteBuffer {}
unsafe impl UserSafeSized for Usercall {}
unsafe impl UserSafeSized for Return {}
unsafe impl<T: UserSafeSized> UserSafeSized for [T; 2] {}

/// A type that can be represented in memory as one or more `UserSafeSized`s.
pub unsafe trait UserSafe {
    unsafe fn align_of() -> usize;

    unsafe fn from_raw_parts_unchecked(ptr: *const u8, size: usize) -> *const Self;

    unsafe fn from_raw_parts(ptr: *const u8, size: usize) -> *const Self {
        let ret = Self::from_raw_parts_unchecked(ptr, size);
        Self::check_ptr(ret);
        ret
    }

    unsafe fn check_ptr(ptr: *const Self) {
        let is_aligned = |p| -> bool {
            0 == (p as usize) & (Self::align_of() - 1)
        };

        assert!(is_aligned(ptr as *const u8));
        assert!(is_user_range(ptr as _, mem::size_of_val(&*ptr)));
        assert!(!ptr.is_null());
    }
}

unsafe impl<T: UserSafeSized> UserSafe for T {
    unsafe fn align_of() -> usize {
        mem::align_of::<T>()
    }

    unsafe fn from_raw_parts_unchecked(ptr: *const u8, size: usize) -> *const Self {
        assert_eq!(size, mem::size_of::<T>());
        ptr as _
    }
}

unsafe impl<T: UserSafeSized> UserSafe for [T] {
    unsafe fn align_of() -> usize {
        mem::align_of::<T>()
    }

    unsafe fn from_raw_parts_unchecked(ptr: *const u8, size: usize) -> *const Self {
        let elem_size = mem::size_of::<T>();
        assert_eq!(size % elem_size, 0);
        let len = size / elem_size;
        slice::from_raw_parts(ptr as _, len)
    }
}

/// A reference to some type in userspace memory. `&UserRef<T>` is equivalent
/// to `&T` in enclave memory. Access to the memory is only allowed by copying
/// to avoid TOCTTOU issues. After copying, code should make sure to completely
/// check the value before use.
pub struct UserRef<T: ?Sized>(UnsafeCell<T>);
/// An owned type in userspace memory. `User<T>` is equivalent to `Box<T>` in
/// enclave memory. Access to the memory is only allowed by copying to avoid
/// TOCTTOU issues. The user memory will be freed when the value is dropped.
/// After copying, code should make sure to completely check the value before
/// use.
pub struct User<T: UserSafe + ?Sized>(*mut UserRef<T>);

impl<T: ?Sized> User<T> where T: UserSafe {
    fn new_uninit_bytes(size: usize) -> Self {
        unsafe {
            let ptr = super::alloc(size, T::align_of()).expect("User memory allocation failed");
            User(T::from_raw_parts(ptr as _, size) as _)
        }
    }

    pub fn new_from_enclave(val: &T) -> Self {
        unsafe {
            let ret = Self::new_uninit_bytes(mem::size_of_val(val));
            ptr::copy(val as *const T as *const u8, ret.0 as *mut T as *mut u8, mem::size_of_val(val));
            ret
        }
    }

    /// Create an owned `User<T>` from a raw pointer. The pointer should be
    /// freeable with the `free` usercall and the alignment of `T`.
    ///
    /// # Panics
    /// This function panics if:
    ///
    /// * The pointer is not aligned
    /// * The pointer is null
    /// * The pointed-to range is not in user memory
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        T::check_ptr(ptr);
        User(ptr as _)
    }

    /// Convert this value into a raw pointer. The value will no longer be
    /// automatically freed.
    pub fn into_raw(self) -> *mut T {
        let ret = self.0;
        mem::forget(self);
        ret as _
    }
}

impl<T> User<T> where T: UserSafe {
    pub fn uninitialized() -> Self {
        Self::new_uninit_bytes(mem::size_of::<T>())
    }
}

impl<T> User<[T]> where [T]: UserSafe {
    pub fn uninitialized(n: usize) -> Self {
        Self::new_uninit_bytes(n * mem::size_of::<T>())
    }

    /// Create an owned `User<[T]>` from a raw thin pointer and a slice length.
    /// The pointer should be freeable with the `free` usercall and the
    /// alignment of `T`.
    ///
    /// # Panics
    /// This function panics if:
    ///
    /// * The pointer is not aligned
    /// * The pointer is null
    /// * The pointed-to range is not in user memory
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        User(<[T]>::from_raw_parts(ptr as _, len) as _)
    }
}

impl<T: ?Sized> UserRef<T> where T: UserSafe {
    /// Create a `&UserRef<[T]>` from a raw pointer.
    ///
    /// # Panics
    /// This function panics if:
    ///
    /// * The pointer is not aligned
    /// * The pointer is null
    /// * The pointed-to range is not in user memory
    pub unsafe fn from_ptr<'a>(ptr: *const T) -> &'a Self {
        T::check_ptr(ptr);
        &*(ptr as *const Self)
    }

    /// Create a `&mut UserRef<[T]>` from a raw pointer.
    ///
    /// # Panics
    /// This function panics if:
    ///
    /// * The pointer is not aligned
    /// * The pointer is null
    /// * The pointed-to range is not in user memory
    pub unsafe fn from_mut_ptr<'a>(ptr: *mut T) -> &'a mut Self {
        T::check_ptr(ptr);
        &mut*(ptr as *mut Self)
    }

    /// # Panics
    /// This function panics if the destination doesn't have the same size as
    /// the source. This can happen for dynamically-sized types such as slices.
    pub fn copy_from_enclave(&mut self, val: &T) {
        unsafe {
            assert_eq!(mem::size_of_val(val), mem::size_of_val( &*self.0.get() ));
            ptr::copy(val as *const T as *const u8, self.0.get() as *mut T as *mut u8, mem::size_of_val(val));
        }
    }

    /// # Panics
    /// This function panics if the destination doesn't have the same size as
    /// the source. This can happen for dynamically-sized types such as slices.
    pub fn copy_to_enclave(&self, dest: &mut T) {
        unsafe {
            assert_eq!(mem::size_of_val(dest), mem::size_of_val( &*self.0.get() ));
            ptr::copy(self.0.get() as *const T as *const u8, dest as *mut T as *mut u8, mem::size_of_val(dest));
        }
    }

    pub fn as_raw_ptr(&self) -> *const T {
        self as *const _ as _
    }

    pub fn as_raw_mut_ptr(&mut self) -> *mut T {
        self as *mut _ as _
    }
}

impl<T> UserRef<T> where T: UserSafe {
    pub fn to_enclave(&self) -> T {
        unsafe { ptr::read(self.0.get()) }
    }
}

impl<T> UserRef<[T]> where [T]: UserSafe {
    /// Create a `&UserRef<[T]>` from a raw thin pointer and a slice length.
    ///
    /// # Panics
    /// This function panics if:
    ///
    /// * The pointer is not aligned
    /// * The pointer is null
    /// * The pointed-to range is not in user memory
    pub unsafe fn from_raw_parts<'a>(ptr: *const T, len: usize) -> &'a Self {
        &*(<[T]>::from_raw_parts(ptr as _, len * mem::size_of::<T>()) as *const Self)
    }

    /// Create a `&mut UserRef<[T]>` from a raw thin pointer and a slice length.
    ///
    /// # Panics
    /// This function panics if:
    ///
    /// * The pointer is not aligned
    /// * The pointer is null
    /// * The pointed-to range is not in user memory
    pub unsafe fn from_raw_parts_mut<'a>(ptr: *mut T, len: usize) -> &'a mut Self {
        &mut*(<[T]>::from_raw_parts(ptr as _, len * mem::size_of::<T>()) as *mut Self)
    }

    pub fn as_ptr(&self) -> *const T {
        self.0.get() as _
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.0.get() as _
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.0.get()).len() }
    }

    pub fn copy_to_enclave_vec(&self, dest: &mut Vec<T>) {
        unsafe {
            if let Some(missing) = self.len().checked_sub(dest.capacity()) {
                dest.reserve(missing)
            }
            dest.set_len(self.len());
            self.copy_to_enclave(&mut dest[..]);
        }
    }

    pub fn to_enclave(&self) -> Vec<T> {
        let mut ret = Vec::with_capacity(self.len());
        self.copy_to_enclave_vec(&mut ret);
        ret
    }
}

impl<T: ?Sized> Deref for User<T> where T: UserSafe {
    type Target = UserRef<T>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<T: ?Sized> DerefMut for User<T> where T: UserSafe {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut*self.0 }
    }
}

impl<T: ?Sized> Drop for User<T> where T: UserSafe {
    fn drop(&mut self) {
        unsafe {
            let ptr = (*self.0).0.get();
            super::free(ptr as _, mem::size_of_val(&mut*ptr), T::align_of());
        }
    }
}

impl<T: CoerceUnsized<U>, U> CoerceUnsized<UserRef<U>> for UserRef<T> {}

impl<T, I: SliceIndex<[T]>> Index<I> for UserRef<[T]> where [T]: UserSafe, I::Output: UserSafe {
    type Output = UserRef<I::Output>;

    #[inline]
    fn index(&self, index: I) -> &UserRef<I::Output> {
        unsafe {
            UserRef::from_ptr(index.index(&*self.as_raw_ptr()))
        }
    }
}

impl<T, I: SliceIndex<[T]>> IndexMut<I> for UserRef<[T]> where [T]: UserSafe, I::Output: UserSafe {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut UserRef<I::Output> {
        unsafe {
            UserRef::from_mut_ptr(index.index_mut(&mut*self.as_raw_mut_ptr()))
        }
    }
}
