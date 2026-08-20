use std::path::PathBuf;

use bytes::Bytes;
use potree::asset::PotreeAsset;
use thiserror::Error;

use crate::{ByteSource, ByteSourceError, FileSource, HttpSource};

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

impl PotreeAssetSource<HttpSource> {
    pub fn from_url(url: &str) -> Result<PotreeAssetSource<HttpSource>, ByteSourceError> {
        let base_url = if url.ends_with('/') {
            // remove leading /
            url.trim_end_matches('/').to_string()
        } else {
            match url.rfind('/') {
                // remove last part of the url if it ends with (metadata.json, hierarchy.bin or
                // octree.bin)
                Some(index) => {
                    let (path, end) = url.split_at(index);
                    match &end[1..] {
                        "metadata.json" | "hierarchy.bin" | "octree.bin" => path.to_string(),
                        _ => url.to_string(),
                    }
                }
                None => url.to_string(),
            }
        };

        Ok(PotreeAssetSource {
            metadata: HttpSource::open(&format!("{}/metadata.json", base_url))?,
            hierarchy: HttpSource::open(&format!("{}/hierarchy.bin", base_url))?,
            octree: HttpSource::open(&format!("{}/octree.bin", base_url))?,
        })
    }
}
