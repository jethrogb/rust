// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Do not remove inline: will result in relocation failure
#[inline(always)]
pub unsafe fn rel_ptr<T>(offset: u64) -> *const T {
    (image_base()+offset) as *const T
}

// Do not remove inline: will result in relocation failure
#[inline(always)]
pub unsafe fn rel_ptr_mut<T>(offset: u64) -> *mut T {
    (image_base()+offset) as *mut T
}

// Do not remove inline: will result in relocation failure
// For the same reason we use inline ASM here instead of an extern static to
// locate the base
#[inline(always)]
fn image_base() -> u64 {
    let base;
    unsafe{asm!("lea IMAGE_BASE(%rip),$0":"=r"(base))};
    base
}
