use std::path::PathBuf;

use async_trait::async_trait;
use bytes::Bytes;
use potree::asset::PotreeAsset;
use thiserror::Error;

use crate::{ByteSource, ByteSourceError, FileSource};

pub struct PotreeAssetSource<S: ByteSource> {
    metadata: S,
    hierarchy: S,
    octree: S,
}

#[derive(Debug, Error)]
pub enum PotreeAssetSourceError {
    #[error(transparent)]
    ByteSource(#[from] ByteSourceError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[async_trait]
impl<S: ByteSource> PotreeAsset for PotreeAssetSource<S> {
    type Error = PotreeAssetSourceError;

    async fn read_metadata(&self) -> Result<Bytes, Self::Error> {
        let buffer = self.metadata.read_to_end(0).await?;
        Ok(buffer.into())
    }

    async fn read_hierarchy(&self, offset: u64, length: usize) -> Result<Bytes, Self::Error> {
        let buffer = self.hierarchy.read_range(offset, length as u64).await?;
        Ok(buffer.into())
    }

    async fn read_octree(&self, offset: u64, length: usize) -> Result<Bytes, Self::Error> {
        let buffer = self.octree.read_range(offset, length as u64).await?;
        Ok(buffer.into())
    }
}

impl PotreeAssetSource<FileSource> {
    pub fn from_path(
        path: impl Into<PathBuf>,
    ) -> Result<PotreeAssetSource<FileSource>, ByteSourceError> {
        let path: PathBuf = path.into();

        Ok(PotreeAssetSource {
            metadata: FileSource::open(path.join("metadata.json"))?,
            hierarchy: FileSource::open(path.join("hierarchy.bin"))?,
            octree: FileSource::open(path.join("octree.bin"))?,
        })
    }
}
