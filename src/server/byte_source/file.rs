// Copied from https://github.com/360-geo/copc/blob/master/copc-streaming/src/file_source.rs

//! File-based [`ByteSource`] for local file access.

use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::Mutex,
};

use super::{ByteSource, ByteSourceError};

/// A [`ByteSource`] backed by a local file.
///
/// Uses a Mutex for interior mutability since Read+Seek requires &mut self
/// but [`ByteSource`]'s [`ByteSource::read_range`] takes &self.
pub struct FileSource {
    file: Mutex<std::fs::File>,
    size: u64,
}

impl FileSource {
    /// Open a local file as a byte source.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ByteSourceError> {
        let file = std::fs::File::open(path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            file: Mutex::new(file),
            size,
        })
    }
}

impl ByteSource for FileSource {
    async fn read_to_end(&self, offset: u64) -> Result<Vec<u8>, ByteSourceError> {
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; (self.size - offset) as usize];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    async fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>, ByteSourceError> {
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; length as usize];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    async fn size(&self) -> Result<Option<u64>, ByteSourceError> {
        Ok(Some(self.size))
    }
}
