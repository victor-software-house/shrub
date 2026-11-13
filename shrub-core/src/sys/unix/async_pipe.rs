//! Async pipe reading utilities for Unix.

use std::fs::File;
use std::io;
use std::io::PipeReader;
use std::os::unix::io::OwnedFd;

use tokio::net::unix::pipe;

pub(crate) struct AsyncPipeReader(pipe::Receiver);

impl AsyncPipeReader {
    pub(crate) fn new(reader: PipeReader) -> io::Result<Self> {
        Ok(Self(pipe::Receiver::from_file(File::from(OwnedFd::from(
            reader,
        )))?))
    }

    pub(crate) async fn read_to_string(&mut self) -> io::Result<String> {
        use tokio::io::AsyncReadExt;
        let mut s = String::new();
        self.0.read_to_string(&mut s).await?;
        Ok(s)
    }
}
