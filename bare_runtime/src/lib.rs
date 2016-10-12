extern crate alloc_buddy_simple;
extern crate rlibc;

#[no_mangle]
pub unsafe extern "C" fn init_heap() {
	use alloc_buddy_simple::{FreeBlock, initialize_allocator};

	static mut FREE_LISTS: [*mut FreeBlock; 19] = [0 as *mut _; 19];

	extern {
		static mut HEAP_BOTTOM: u8;
		static mut HEAP_TOP: u8;
	}

	let heap_bottom_ptr = &mut HEAP_BOTTOM as *mut _;
	let heap_top_ptr = &mut HEAP_TOP as *mut _;
	let heap_size = heap_top_ptr as usize - heap_bottom_ptr as usize;
	initialize_allocator(heap_bottom_ptr, heap_size, &mut FREE_LISTS);
}

// fix linker errors
pub fn use_rlibc_fns() -> (
		unsafe extern fn(dest: *mut u8, src: *const u8, n: usize) -> *mut u8,
		unsafe extern fn(dest: *mut u8, src: *const u8, n: usize) -> *mut u8,
		unsafe extern fn(s: *mut u8, c: i32, n: usize) -> *mut u8,
		unsafe extern fn(s1: *const u8, s2: *const u8, n: usize) -> i32,
	) {
	(rlibc::memcpy,rlibc::memmove,rlibc::memset,rlibc::memcmp)
}
