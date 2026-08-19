use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{Stream, StreamExt, TryStreamExt};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use quick_xml::events::Event;
use reqwest::{Method, StatusCode};
use tokio_util::io::{ReaderStream, StreamReader};

use crate::backend::{Backend, Entry, EntryKind};
use crate::error::FpError;
use crate::rpath;
use crate::transfer::{copy_with_progress, TransferCtl, TransferResult};

/// Alles außer unreserved chars und `/` in Pfadsegmenten encodieren.
const PATH_ENC: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'%')
    .add(b'[')
    .add(b']')
    .add(b'|')
    .add(b'\\')
    .add(b'^');

pub struct WebdavConfig {
    /// Basis-URL der DAV-Wurzel, z. B.
    /// `https://cloud.example.com/remote.php/dav/files/anna`.
    pub base_url: String,
    pub user: String,
    pub password: String,
    pub accept_invalid_certs: bool,
}

pub struct WebdavBackend {
    client: reqwest::Client,
    base: String,
    /// Pfad-Anteil der Basis-URL — wird von zurückgelieferten hrefs abgeschnitten.
    base_path: String,
    user: String,
    password: String,
    label: String,
}

fn map_status(path: &str, status: StatusCode) -> FpError {
    match status {
        StatusCode::NOT_FOUND => FpError::NotFound(path.to_string()),
        StatusCode::UNAUTHORIZED => FpError::Auth("Zugangsdaten abgelehnt (401)".into()),
        StatusCode::FORBIDDEN => FpError::Denied(path.to_string()),
        s => FpError::Protocol(format!("{path}: HTTP {s}")),
    }
}

impl WebdavBackend {
    pub async fn connect(cfg: WebdavConfig) -> Result<Self, FpError> {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(cfg.accept_invalid_certs)
            .build()
            .map_err(|e| FpError::Connect(e.to_string()))?;
        let base = cfg.base_url.trim_end_matches('/').to_string();
        let parsed = reqwest::Url::parse(&base)
            .map_err(|e| FpError::Connect(format!("Ungültige URL: {e}")))?;
        let host = parsed.host_str().unwrap_or("").to_string();
        let base_path = percent_decode_str(parsed.path())
            .decode_utf8_lossy()
            .trim_end_matches('/')
            .to_string();
        let be = WebdavBackend {
            client,
            base,
            base_path,
            user: cfg.user.clone(),
            password: cfg.password,
            label: format!("dav://{}@{}", cfg.user, host),
        };
        // Verbindung + Zugangsdaten sofort prüfen, nicht erst beim Browsen.
        be.list("/").await?;
        Ok(be)
    }

    fn url(&self, path: &str) -> String {
        let enc = utf8_percent_encode(path.trim_start_matches('/'), PATH_ENC);
        format!("{}/{}", self.base, enc)
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, self.url(path))
            .basic_auth(&self.user, Some(&self.password))
    }
}

/// Zieht aus einem `multistatus`-PROPFIND die Einträge unterhalb von `dir`.
fn parse_propfind(xml: &str, base_path: &str, dir: &str) -> Result<Vec<Entry>, FpError> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    #[derive(Default)]
    struct Cur {
        href: String,
        is_dir: bool,
        size: u64,
        modified: Option<i64>,
    }

    let mut entries = Vec::new();
    let mut cur: Option<Cur> = None;
    let mut text_target: Option<&'static str> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.local_name();
                let tag = String::from_utf8_lossy(name.as_ref()).to_lowercase();
                match tag.as_str() {
                    "response" => cur = Some(Cur::default()),
                    "collection" => {
                        if let Some(c) = cur.as_mut() {
                            c.is_dir = true;
                        }
                    }
                    "href" => text_target = Some("href"),
                    "getcontentlength" => text_target = Some("size"),
                    "getlastmodified" => text_target = Some("modified"),
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let (Some(target), Some(c)) = (text_target, cur.as_mut()) {
                    let text = t.decode().unwrap_or_default().to_string();
                    match target {
                        "href" => c.href = text,
                        "size" => c.size = text.trim().parse().unwrap_or(0),
                        "modified" => {
                            c.modified = httpdate::parse_http_date(text.trim()).ok().and_then(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .ok()
                                    .map(|d| d.as_secs() as i64)
                            })
                        }
                        _ => {}
                    }
                }
                text_target = None;
            }
            Ok(Event::End(e)) => {
                let name = e.local_name();
                if name.as_ref().eq_ignore_ascii_case(b"response") {
                    if let Some(c) = cur.take() {
                        // href → Pfad relativ zur DAV-Wurzel
                        let href = c.href.split("://").last().unwrap_or(&c.href);
                        let raw_path = match href.find('/') {
                            Some(idx) if c.href.contains("://") => &href[idx..],
                            _ => href,
                        };
                        let decoded = percent_decode_str(raw_path).decode_utf8_lossy();
                        let mut rel = decoded
                            .strip_prefix(&*format!("{base_path}/"))
                            .map(|r| format!("/{r}"))
                            .or_else(|| (decoded.trim_end_matches('/') == base_path).then(|| "/".to_string()))
                            .unwrap_or_else(|| decoded.to_string());
                        if rel.len() > 1 {
                            rel = rel.trim_end_matches('/').to_string();
                        }
                        // Das Verzeichnis selbst taucht in der Antwort mit auf — überspringen.
                        if rel != dir && rel != "/" || (dir != "/" && rel == "/") {
                            if rel != dir {
                                entries.push(Entry {
                                    name: rpath::file_name(&rel).to_string(),
                                    path: rel,
                                    kind: if c.is_dir { EntryKind::Dir } else { EntryKind::File },
                                    size: c.size,
                                    modified: c.modified,
                                });
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(FpError::Protocol(format!("PROPFIND-Antwort: {e}"))),
            _ => {}
        }
        buf.clear();
    }
    Ok(entries)
}

/// Umhüllt einen Datei-Stream für PUT: meldet Fortschritt, hasht die
/// Bytes und bricht ab, wenn das Token gekündigt wird.
struct ProgressStream {
    inner: ReaderStream<tokio::fs::File>,
    ctl: TransferCtl,
    total: Option<u64>,
    done: u64,
    hasher: Arc<Mutex<blake3::Hasher>>,
}

impl Stream for ProgressStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.ctl.cancel.is_cancelled() {
            return Poll::Ready(Some(Err(std::io::Error::other("abgebrochen"))));
        }
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                self.hasher.lock().unwrap().update(&chunk);
                self.done += chunk.len() as u64;
                (self.ctl.progress)(self.done, self.total);
                Poll::Ready(Some(Ok(chunk)))
            }
            other => other,
        }
    }
}

#[async_trait]
impl Backend for WebdavBackend {
    fn label(&self) -> String {
        self.label.clone()
    }

    async fn initial_dir(&self) -> Result<String, FpError> {
        Ok("/".to_string())
    }

    async fn list(&self, path: &str) -> Result<Vec<Entry>, FpError> {
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop><d:resourcetype/><d:getcontentlength/><d:getlastmodified/></d:prop>
</d:propfind>"#;
        let resp = self
            .request(Method::from_bytes(b"PROPFIND").unwrap(), path)
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(body)
            .send()
            .await
            .map_err(|e| FpError::Connect(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(map_status(path, resp.status()));
        }
        let xml = resp.text().await.map_err(FpError::proto)?;
        parse_propfind(&xml, &self.base_path, path.trim_end_matches('/'))
    }

    async fn download(
        &self,
        remote: &str,
        local: &Path,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let resp = self
            .request(Method::GET, remote)
            .send()
            .await
            .map_err(|e| FpError::Connect(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(map_status(remote, resp.status()));
        }
        let total = resp.content_length();
        let stream = resp.bytes_stream().map_err(std::io::Error::other);
        let reader = StreamReader::new(stream);
        let dst = tokio::fs::File::create(local).await?;
        copy_with_progress(reader, dst, total, ctl).await
    }

    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let src = tokio::fs::File::open(local).await?;
        let total = src.metadata().await.ok().map(|m| m.len());
        let hasher = Arc::new(Mutex::new(blake3::Hasher::new()));
        let stream = ProgressStream {
            inner: ReaderStream::new(src),
            ctl: ctl.clone(),
            total,
            done: 0,
            hasher: hasher.clone(),
        };
        let mut req = self
            .request(Method::PUT, remote)
            .body(reqwest::Body::wrap_stream(stream));
        if let Some(len) = total {
            req = req.header("Content-Length", len);
        }
        let resp = req.send().await.map_err(|e| {
            if ctl.cancel.is_cancelled() {
                FpError::Cancelled
            } else {
                FpError::Connect(e.to_string())
            }
        })?;
        if !resp.status().is_success() {
            return Err(map_status(remote, resp.status()));
        }
        let hasher = hasher.lock().unwrap();
        Ok(TransferResult {
            bytes: total.unwrap_or(0),
            blake3: hasher.finalize().to_hex().to_string(),
        })
    }

    async fn mkdir(&self, path: &str) -> Result<(), FpError> {
        let resp = self
            .request(Method::from_bytes(b"MKCOL").unwrap(), path)
            .send()
            .await
            .map_err(|e| FpError::Connect(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(map_status(path, resp.status()));
        }
        Ok(())
    }

    async fn remove(&self, path: &str, _is_dir: bool) -> Result<(), FpError> {
        let resp = self
            .request(Method::DELETE, path)
            .send()
            .await
            .map_err(|e| FpError::Connect(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(map_status(path, resp.status()));
        }
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FpError> {
        let resp = self
            .request(Method::from_bytes(b"MOVE").unwrap(), from)
            .header("Destination", self.url(to))
            .header("Overwrite", "T")
            .send()
            .await
            .map_err(|e| FpError::Connect(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(map_status(from, resp.status()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_apache_style_multistatus() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/dav/docs/</D:href>
    <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop>
      <D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/docs/Bericht%202026.pdf</D:href>
    <D:propstat><D:prop>
      <D:resourcetype/>
      <D:getcontentlength>12345</D:getcontentlength>
      <D:getlastmodified>Tue, 18 Aug 2026 10:00:00 GMT</D:getlastmodified>
    </D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/docs/unter/</D:href>
    <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop>
      <D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>"#;
        let entries = parse_propfind(xml, "/dav", "/docs").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Bericht 2026.pdf");
        assert_eq!(entries[0].path, "/docs/Bericht 2026.pdf");
        assert_eq!(entries[0].kind, EntryKind::File);
        assert_eq!(entries[0].size, 12345);
        assert!(entries[0].modified.is_some());
        assert_eq!(entries[1].name, "unter");
        assert_eq!(entries[1].kind, EntryKind::Dir);
    }
}
