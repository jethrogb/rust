#![feature(rand,asm)]

use std::io::{Write,Error as IoError,Result as IoResult};
use std::fs::File;
use std::collections::HashMap;

extern crate bare_runtime;

#[inline(always)]
unsafe fn syscall_exit_group() -> ! {
	asm!(
		"syscall"
		:
		: "{rax}"(231)
		:
		: "volatile"
	);
	unreachable!()
}

#[inline(always)]
unsafe fn syscall_write(fd: i32, buf: *const u8, count: usize) -> isize {
	let ret;
	asm!(
		"syscall"
		: "={rax}"(ret)
		: "{rax}"(1), "{edi}"(fd), "{rsi}"(buf), "{rdx}"(count)
		:
		: "volatile"
	);
	ret
}

struct Stdout;

impl Write for Stdout {
	fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
		let ret=unsafe{syscall_write(1,buf.as_ptr(),buf.len())};
		if ret<0 {
			Err(IoError::last_os_error())
		} else {
			Ok(ret as usize)
		}
	}
	
	fn flush(&mut self) -> IoResult<()> { Ok(()) }
}

use std::__rand::{Rng,thread_rng};

fn main() {
	::std::panic::set_hook(Box::new(|info|{
		let msg = match info.payload().downcast_ref::<&'static str>() {
			Some(s) => *s,
			None => match info.payload().downcast_ref::<String>() {
				Some(s) => &s,
				None => "unknown panic payload",
			}
		};

		let _=match info.location() {
			Some(loc) => writeln!(Stdout,"panicked at '{}' in {}:{}", msg, loc.file(), loc.line()),
			None => writeln!(Stdout,"panicked at '{}'", msg),
		};
		unsafe{syscall_exit_group()};
	}));

	let mut rand_buf=vec![0u8;16];
	thread_rng().fill_bytes(&mut rand_buf);
	let _=writeln!(Stdout,"{}",Box::new("Hello heap allocation!"));
	let _=writeln!(Stdout,"HashMap: {:?}",HashMap::<(),()>::new());
	let _=writeln!(Stdout,"So random: {:?}",rand_buf);
	File::open("/dev/null").unwrap();
}
