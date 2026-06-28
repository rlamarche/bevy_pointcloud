use std::{collections::BTreeMap, num::ParseIntError};

use super::{ByteSource, ByteSourceError};

pub struct HttpSource {
    url: String,
}

impl HttpSource {
    pub fn open(url: &str) -> Result<Self, ByteSourceError> {
        Ok(Self {
            url: url.to_string(),
        })
    }
}

impl ByteSource for HttpSource {
    async fn read_to_end(&self, offset: u64) -> Result<Vec<u8>, ByteSourceError> {
        let mut headers = BTreeMap::new();
        if offset > 0 {
            headers.insert("range".into(), format!("bytes={}-", offset));
        }

        ehttp_get(&self.url, Some(headers)).await
    }

    async fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>, ByteSourceError> {
        let end = offset.checked_add(length).map(|v| v - 1).ok_or_else(|| {
            ByteSourceError::ByteSource(HttpSourceError("Range overflow".into()).into())
        })?;

        let mut headers = BTreeMap::new();
        headers.insert("range".into(), format!("bytes={}-{}", offset, end));

        ehttp_get(&self.url, Some(headers)).await
    }

    async fn size(&self) -> Result<Option<u64>, ByteSourceError> {
        ehttp_get_size(&self.url, None).await
    }
}

#[derive(Debug)]
pub struct HttpSourceError(String);

impl std::fmt::Display for HttpSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for HttpSourceError {}

async fn ehttp_get_size(
    url: &str,
    headers: Option<BTreeMap<String, String>>,
) -> Result<Option<u64>, ByteSourceError> {
    let headers = build_headers(headers);
    let request = ehttp::Request {
        method: "HEAD".to_owned(),
        url: url.to_string(),
        body: vec![],
        headers,
        #[cfg(target_arch = "wasm32")]
        mode: ehttp::Mode::default(),
    };

    let response = send_request(request).await?;

    response
        .headers
        .get("Content-Length")
        .map(&str::parse)
        .transpose()
        .map_err(|e: ParseIntError| ByteSourceError::ByteSource(e.into()))
}

async fn ehttp_get(
    url: &str,
    headers: Option<BTreeMap<String, String>>,
) -> Result<Vec<u8>, ByteSourceError> {
    let headers = build_headers(headers);
    let request = ehttp::Request {
        method: "GET".to_owned(),
        url: url.to_string(),
        body: vec![],
        headers,
        #[cfg(target_arch = "wasm32")]
        mode: ehttp::Mode::default(),
    };

    let response = send_request(request).await?;
    Ok(response.bytes)
}

/// Convert an optional header map into `ehttp::Headers`.
fn build_headers(map: Option<BTreeMap<String, String>>) -> ehttp::Headers {
    let mut headers = ehttp::Headers::default();
    if let Some(m) = map {
        for (k, v) in m {
            headers.insert(k, v);
        }
    }
    headers
}

/// Send an `ehttp` request and return the response, mapping errors uniformly.
async fn send_request(request: ehttp::Request) -> Result<ehttp::Response, ByteSourceError> {
    let (tx, rx) = futures::channel::oneshot::channel();
    ehttp::fetch(request, move |res| {
        let _ = tx.send(res);
    });

    let result = rx.await.map_err(|_| {
        ByteSourceError::ByteSource(HttpSourceError("channel closed".into()).into())
    })?;

    let response = result
        .map_err(|e| ByteSourceError::ByteSource(HttpSourceError(format!("{e:?}")).into()))?;

    if !(200..300).contains(&(response.status as usize)) {
        return Err(ByteSourceError::ByteSource(
            HttpSourceError(format!("HTTP {}", response.status)).into(),
        ));
    }

    Ok(response)
}
