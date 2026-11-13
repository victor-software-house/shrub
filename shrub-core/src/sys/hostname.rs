use std::ffi::OsString;
use std::io;

pub(crate) fn get() -> io::Result<OsString> {
    hostname::get()
}
