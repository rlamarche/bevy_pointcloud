// Copied from https://github.com/360-geo/copc/blob/master/copc-streaming/src/byte_source.rs

mod file;
#[cfg(feature = "http_source")]
mod http;

use bevy::tasks::{BoxedFuture, ConditionalSendFuture};
pub use file::*;
#[cfg(feature = "http_source")]
pub use http::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ByteSourceError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Custom error from a [`ByteSource`](crate::ByteSource) implementation.
    #[error("byte source error: {0}")]
    ByteSource(Box<dyn std::error::Error + Send + Sync>),
}

/// Async random-access byte source.
///
/// Implementations can back this with HTTP range requests, local file I/O,
/// in-memory buffers, or any other random-access mechanism.
///
/// All methods return non-Send futures for WASM compatibility.
pub trait ByteSource: Sync + Send + 'static {
    /// Read remaining bytes starting at `offset`.
    fn read_to_end(
        &self,
        offset: u64,
    ) -> impl ConditionalSendFuture<Output = Result<Vec<u8>, ByteSourceError>>;

    /// Read `length` bytes starting at `offset`.
    fn read_range(
        &self,
        offset: u64,
        length: u64,
    ) -> impl ConditionalSendFuture<Output = Result<Vec<u8>, ByteSourceError>>;

    /// Total size of the source in bytes, if known.
    fn size(&self) -> impl ConditionalSendFuture<Output = Result<Option<u64>, ByteSourceError>>;

    /// Read multiple ranges in one logical operation.
    ///
    /// The default implementation issues sequential reads.
    /// HTTP implementations should override to issue parallel requests.
    fn read_ranges(
        &self,
        ranges: &[(u64, u64)],
    ) -> impl ConditionalSendFuture<Output = Result<Vec<Vec<u8>>, ByteSourceError>> {
        async move {
            let mut results = Vec::with_capacity(ranges.len());
            for &(offset, length) in ranges {
                results.push(self.read_range(offset, length).await?);
            }
            Ok(results)
        }
    }
}

/// In-memory byte source, useful for testing and when data is already loaded.
impl ByteSource for Vec<u8> {
    async fn read_to_end(&self, offset: u64) -> Result<Vec<u8>, ByteSourceError> {
        let start = offset as usize;
        Ok(self[start..].to_vec())
    }

    async fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>, ByteSourceError> {
        let start = offset as usize;
        let end = start + length as usize;
        if end > self.len() {
            return Err(ByteSourceError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!(
                    "read_range({offset}, {length}) out of bounds (size {})",
                    self.len()
                ),
            )));
        }
        Ok(self[start..end].to_vec())
    }

    async fn size(&self) -> Result<Option<u64>, ByteSourceError> {
        Ok(Some(self.len() as u64))
    }
}

/// Erased trait for [`ByteSource`] dyn compatible
pub trait ErasedByteSource: Send + Sync + 'static {
    /// Read remaining bytes starting at `offset`.
    fn read_to_end<'a>(&'a self, offset: u64) -> BoxedFuture<'a, Result<Vec<u8>, ByteSourceError>>;

    /// Read `length` bytes starting at `offset`.
    fn read_range<'a>(
        &'a self,
        offset: u64,
        length: u64,
    ) -> BoxedFuture<'a, Result<Vec<u8>, ByteSourceError>>;

    /// Total size of the source in bytes, if known.
    fn size<'a>(&'a self) -> BoxedFuture<'a, Result<Option<u64>, ByteSourceError>>;

    /// Read multiple ranges in one logical operation.
    ///
    /// The default implementation issues sequential reads.
    /// HTTP implementations should override to issue parallel requests.
    fn read_ranges<'a>(
        &'a self,
        ranges: Vec<(u64, u64)>,
    ) -> BoxedFuture<'a, Result<Vec<Vec<u8>>, ByteSourceError>>;
}

impl<S: ByteSource> ErasedByteSource for S {
    fn read_to_end<'a>(&'a self, offset: u64) -> BoxedFuture<'a, Result<Vec<u8>, ByteSourceError>> {
        Box::pin(self.read_to_end(offset))
    }

    fn read_range<'a>(
        &'a self,
        offset: u64,
        length: u64,
    ) -> BoxedFuture<'a, Result<Vec<u8>, ByteSourceError>> {
        Box::pin(self.read_range(offset, length))
    }

    fn size<'a>(&'a self) -> BoxedFuture<'a, Result<Option<u64>, ByteSourceError>> {
        Box::pin(self.size())
    }

    fn read_ranges<'a>(
        &'a self,
        ranges: Vec<(u64, u64)>,
    ) -> BoxedFuture<'a, Result<Vec<Vec<u8>>, ByteSourceError>> {
        Box::pin(async move { self.read_ranges(&ranges).await })
    }
}
