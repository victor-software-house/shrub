use std::ffi::OsString;
use std::io;

use crate::sys::hostname;

pub(crate) fn get_hostname() -> io::Result<OsString> {
    hostname::get()
}
