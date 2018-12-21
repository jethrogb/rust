// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![unstable(feature = "unix_socket_abstract", issue = "42048")]

use ffi::OsStr;
use io;
use os::unix::ffi::OsStrExt;
use os::unix::net::{sockaddr_un_abstract, SocketAddr, AddressKind};

/// Linux-specific extensions to [`unix::net::SocketAddr`].
///
/// [`unix::net::SocketAddr`]: ../../unix/net/struct.SocketAddr.html
#[unstable(feature = "unix_socket_abstract", issue = "42048")]
pub trait SocketAddrExt: Sized {
    /// Creates a new socket address from an abstract address `address`.
    ///
    /// `address` should *not* have a preceding null byte to indicate it's an
    /// abstract address. The address must fit in the platform's socket address
    /// representation.
    fn new_abstract<A: AsRef<OsStr>>(address: A) -> io::Result<Self>;

    /// Returns the contents of this address if it is an abstract address.
    fn as_abstract(&self) -> Option<&OsStr>;
}

#[unstable(feature = "unix_socket_abstract", issue = "42048")]
impl SocketAddrExt for SocketAddr {
    fn new_abstract<A: AsRef<OsStr>>(address: A) -> io::Result<Self> {
        unsafe {
            let (addr, len) = sockaddr_un_abstract(address.as_ref())?;
            SocketAddr::from_parts(addr, len)
        }
    }

    fn as_abstract(&self) -> Option<&OsStr> {
        if let AddressKind::Abstract(bytes) = self.address() {
            Some(OsStr::from_bytes(bytes))
        } else {
            None
        }
    }
}
