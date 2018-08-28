// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use io::{self, Write};
use slice::from_raw_parts_mut;

pub struct SgxPanicOutput(&'static mut [u8]);

impl Write for SgxPanicOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

// FIXME: would prefer to use `impl Write` instead of a custom newtype, but
// https://github.com/rust-lang/rust/pull/53545 made this impossible.
pub fn panic_output() -> Option<SgxPanicOutput> {
    extern "C" { fn take_debug_panic_buf_ptr() -> *mut u8; }

    unsafe {
        let ptr = take_debug_panic_buf_ptr();
        if ptr.is_null() {
            None
        } else {
            Some(SgxPanicOutput(from_raw_parts_mut(ptr, 1024)))
        }
    }
}

#[unstable(feature = "perma_unstable", issue = "0")]
#[doc(hidden)]
#[no_mangle]
pub extern "C" fn panic_msg(msg: &str) -> ! {
    let _ = panic_output().map(|mut w| w.write(msg.as_bytes()));
    unsafe { panic_exit(); }
}

extern "C" { pub fn panic_exit() -> !; }
