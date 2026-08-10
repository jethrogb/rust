//! Standalone Miri harness for `std::sys::pal::sgx::waitqueue::unsafe_list`.
//!
//! `src/orig.rs` and `src/fix-aliasing.rs` are mechanical ports
//! (`use crate::` -> `use std::`, `rtassert!` -> `assert!`, `mod tests`
//! hoisted to the crate root) of `unsafe_list.rs` before and after the
//! aliasing fix for rust-lang/rust#160603:
//!
//! * default: the original code (parent commit f73951df0a5)
//! * `--features fix`: the fixed code (commit ed40f99156f)
//!
//! `src/tests.rs` holds the in-tree unit tests (identical at both commits).
//! Run e.g.:
//!
//! ```sh
//! cargo +nightly miri test                  # original, Stacked Borrows
//! cargo +nightly miri test --features fix   # fixed, Stacked Borrows
//! MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --features fix
//! ```

#[cfg_attr(not(feature = "fix"), path = "orig.rs")]
#[cfg_attr(feature = "fix", path = "fix-aliasing.rs")]
pub mod unsafe_list;

#[cfg(test)]
mod tests;

// Regression tests for rust-lang/rust#160603: exercise the aliasing patterns
// used by the SGX `WaitQueue`. `hostile_reborrow` simulates safe code outside
// the module taking `&mut` to the structure containing the list (as
// `SpinMutexGuard::deref_mut` and `WaitVariable::lock_var_mut` do), which
// invalidates any head/tail pointer derived during a previous operation.
#[cfg(test)]
mod regression {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;

    use crate::unsafe_list::*;

    struct Wrapper {
        list: UnsafeList<i32>,
        other: u32,
    }

    fn hostile_reborrow(w: &mut Wrapper) {
        w.other = w.other.wrapping_add(1);
    }

    // The `wait_timeout` fallback path: push an entry, use the returned
    // reference, then remove the entry. The head/tail links loaded in
    // `remove` (both `prev` and `next` here) must not be dereferenced with
    // stale provenance.
    #[test]
    fn wait_timeout_fallback() {
        unsafe {
            let mut w = Wrapper { list: UnsafeList::new(), other: 0 };
            let mut entry = UnsafeListEntry::new(1234);
            let value = w.list.push(&mut entry);
            assert_eq!(*value, 1234);

            hostile_reborrow(&mut w);

            // Not woken up: remove our own entry, as `wait_timeout` does.
            w.list.remove(&mut entry);
            assert!(w.list.pop().is_none());
        }
    }

    // Removing the first entry while others are present: with the original
    // code, `remove(&mut entry)` invalidated the head/tail-adjacent pointers
    // when `entry` was the first element.
    #[test]
    fn remove_first_of_many() {
        unsafe {
            let mut e1 = UnsafeListEntry::new(1);
            let mut e2 = UnsafeListEntry::new(2);
            let mut e3 = UnsafeListEntry::new(3);
            let mut list = UnsafeList::new();
            list.push(&mut e1);
            list.push(&mut e2);
            list.push(&mut e3);
            list.remove(&mut e1);
            assert_eq!(list.pop().unwrap(), &2);
            assert_eq!(list.pop().unwrap(), &3);
            assert!(list.pop().is_none());
        }
    }

    // Entries pushed from different "stack frames" and popped by a
    // "notifier" (like `notify_all`), with hostile reborrows between every
    // operation. Exercises the head/tail `prev` link loaded in `push` and
    // the head/tail `next` link loaded when popping the last entry.
    #[test]
    fn notify_all_pattern() {
        unsafe {
            let mut w = Wrapper { list: UnsafeList::new(), other: 0 };
            let mut e1 = UnsafeListEntry::new(1);
            let mut e2 = UnsafeListEntry::new(2);
            w.list.push(&mut e1);
            hostile_reborrow(&mut w);
            w.list.push(&mut e2);
            hostile_reborrow(&mut w);

            let mut count = 0;
            while let Some(v) = w.list.pop() {
                count += *v;
                hostile_reborrow(&mut w);
            }
            assert_eq!(count, 3);
        }
    }

    // Cross-thread `wait`/`notify_one` pattern: the waiting thread pushes a
    // stack-allocated entry and keeps reading through the `&T` returned by
    // `push` while the notifying thread pops the entry — whose whole-entry
    // accesses may overlap the entry's `value` field — and stores through
    // the reference returned by `pop`. The value is interior-mutable, like
    // the real `SpinMutex<WaitEntry>`. The `Mutex` guards' `deref_mut`
    // provides the hostile reborrows of the structure containing the list.
    #[test]
    fn cross_thread_wait_notify() {
        struct Queue {
            list: UnsafeList<AtomicBool>,
        }
        // SAFETY: like the real `WaitQueue`, the list is only accessed while
        // holding the mutex.
        unsafe impl Send for Queue {}

        let queue = Arc::new(Mutex::new(Queue { list: UnsafeList::new() }));
        for _ in 0..3 {
            let waiter = {
                let queue = Arc::clone(&queue);
                thread::spawn(move || unsafe {
                    let mut entry = UnsafeListEntry::new(AtomicBool::new(false));
                    let wake = queue.lock().unwrap().list.push(&mut entry);
                    while !wake.load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    // `entry` is dropped here: the notifier popped it before
                    // setting `wake` and no longer accesses it.
                })
            };
            loop {
                let mut guard = queue.lock().unwrap();
                if let Some(wake) = unsafe { guard.list.pop() } {
                    // Set under the queue lock, like `notify_one`.
                    wake.store(true, Ordering::Release);
                    break;
                }
                drop(guard);
                thread::yield_now();
            }
            waiter.join().unwrap();
        }
    }

    // Empty-list churn: repeated push/pop cycles with reborrows in between,
    // so each operation sees only stale head/tail self-links.
    #[test]
    fn empty_churn() {
        unsafe {
            let mut w = Wrapper { list: UnsafeList::new(), other: 0 };
            for i in 0..4 {
                let mut e = UnsafeListEntry::new(i);
                w.list.push(&mut e);
                hostile_reborrow(&mut w);
                assert_eq!(w.list.pop().unwrap(), &i);
                hostile_reborrow(&mut w);
                assert!(w.list.is_empty());
            }
        }
    }
}
