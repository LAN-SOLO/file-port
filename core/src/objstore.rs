//! Objektspeicher-Backends über `object_store`: S3-kompatible Dienste,
//! Azure Blob Storage und Google Cloud Storage teilen sich dieselbe
//! [`Backend`]-Implementierung — nur der Verbindungsaufbau unterscheidet sich.
//! Alle Verbindungen laufen ausschließlich über HTTPS.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::TryStreamExt;
use object_store::aws::AmazonS3Builder;
use object_store::azure::MicrosoftAzureBuilder;
use object_store::buffered::BufWriter;
use object_store::gcp::GoogleCloudStorageBuilder;
use object_store::path::Path as ObjPath;
use object_store::{ObjectStore, ObjectStoreExt};
use tokio::io::AsyncWriteExt;
use tokio_util::io::StreamReader;

use crate::backend::{Backend, Entry, EntryKind};
use crate::error::FpError;
use crate::transfer::{copy_with_progress, TransferCtl, TransferResult};

pub struct S3Config {
    /// Leer für AWS; sonst z. B. `https://s3.eu-central-003.backblazeb2.com`
    /// (Backblaze B2) oder `https://fsn1.your-objectstorage.com` (Hetzner).
    pub endpoint: Option<String>,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    /// Path-Style-Adressierung erzwingen (MinIO & Co. ohne Wildcard-DNS).
    pub path_style: bool,
    /// Selbstsignierte Zertifikate akzeptieren (selbst gehostete Endpoints) —
    /// bewusste Entscheidung der Nutzerin im Verbindungs-Profil.
    pub accept_invalid_certs: bool,
}

pub struct AzureConfig {
    /// Name des Storage-Kontos (`https://<account>.blob.core.windows.net`).
    pub account: String,
    pub container: String,
    pub access_key: String,
}

pub struct GcsConfig {
    pub bucket: String,
    /// Pfad zur Service-Account-Schlüsseldatei (JSON).
    pub key_file: String,
}

pub struct ObjectBackend {
    store: Arc<dyn ObjectStore>,
    label: String,
}

fn map_obj(path: &str, e: object_store::Error) -> FpError {
    match e {
        object_store::Error::NotFound { .. } => FpError::NotFound(path.to_string()),
        object_store::Error::PermissionDenied { .. } | object_store::Error::Unauthenticated { .. } => {
            FpError::Auth(format!("{path}: Zugriff abgelehnt"))
        }
        other => FpError::Protocol(other.to_string()),
    }
}

/// UI-Pfad (`/a/b`) → Objektschlüssel (`a/b`).
fn key(path: &str) -> ObjPath {
    ObjPath::from(path.trim_matches('/'))
}

/// Name des unsichtbaren Ordner-Markers (siehe [`ObjectBackend::mkdir`]).
const DIR_MARKER: &str = ".fileport-dir";

impl ObjectBackend {
    pub async fn connect_s3(cfg: S3Config) -> Result<Self, FpError> {
        let mut builder = AmazonS3Builder::new()
            .with_client_options(
                object_store::ClientOptions::new()
                    .with_allow_invalid_certificates(cfg.accept_invalid_certs),
            )
            .with_region(&cfg.region)
            .with_bucket_name(&cfg.bucket)
            .with_access_key_id(&cfg.access_key)
            .with_secret_access_key(&cfg.secret_key)
            .with_virtual_hosted_style_request(!cfg.path_style);
        if let Some(endpoint) = &cfg.endpoint {
            // `http://` existiert allein für die Integrationstests gegen
            // lokales MinIO — die App lässt nur `https://`-Endpoints zu.
            builder = builder
                .with_endpoint(endpoint.trim_end_matches('/'))
                .with_allow_http(endpoint.starts_with("http://"));
        }
        let store = builder.build().map_err(|e| FpError::Connect(e.to_string()))?;
        Self::verify(ObjectBackend {
            store: Arc::new(store),
            label: format!("s3://{}", cfg.bucket),
        })
        .await
    }

    pub async fn connect_azure(cfg: AzureConfig) -> Result<Self, FpError> {
        let store = MicrosoftAzureBuilder::new()
            .with_account(&cfg.account)
            .with_container_name(&cfg.container)
            .with_access_key(&cfg.access_key)
            .build()
            .map_err(|e| FpError::Connect(e.to_string()))?;
        Self::verify(ObjectBackend {
            store: Arc::new(store),
            label: format!("az://{}/{}", cfg.account, cfg.container),
        })
        .await
    }

    pub async fn connect_gcs(cfg: GcsConfig) -> Result<Self, FpError> {
        let store = GoogleCloudStorageBuilder::new()
            .with_bucket_name(&cfg.bucket)
            .with_service_account_path(&cfg.key_file)
            .build()
            .map_err(|e| FpError::Connect(e.to_string()))?;
        Self::verify(ObjectBackend {
            store: Arc::new(store),
            label: format!("gs://{}", cfg.bucket),
        })
        .await
    }

    /// Zugangsdaten und Bucket/Container sofort prüfen, nicht erst beim Browsen.
    async fn verify(be: Self) -> Result<Self, FpError> {
        be.list("/").await?;
        Ok(be)
    }
}

#[async_trait]
impl Backend for ObjectBackend {
    fn label(&self) -> String {
        self.label.clone()
    }

    async fn initial_dir(&self) -> Result<String, FpError> {
        Ok("/".to_string())
    }

    async fn list(&self, path: &str) -> Result<Vec<Entry>, FpError> {
        let prefix = path.trim_matches('/');
        let obj_prefix = (!prefix.is_empty()).then(|| ObjPath::from(prefix));
        let result = self
            .store
            .list_with_delimiter(obj_prefix.as_ref())
            .await
            .map_err(|e| map_obj(path, e))?;

        let mut out = Vec::new();
        for p in result.common_prefixes {
            let name = p.parts().last().map(|s| s.as_ref().to_string()).unwrap_or_default();
            out.push(Entry {
                path: format!("/{p}"),
                name,
                kind: EntryKind::Dir,
                size: 0,
                modified: None,
            });
        }
        for meta in result.objects {
            let name = meta
                .location
                .parts()
                .last()
                .map(|s| s.as_ref().to_string())
                .unwrap_or_default();
            // Ordner-Marker und das Präfix selbst nicht als Dateien zeigen
            if name.is_empty() || name == DIR_MARKER || meta.location.as_ref() == prefix {
                continue;
            }
            out.push(Entry {
                path: format!("/{}", meta.location),
                name,
                kind: EntryKind::File,
                size: meta.size,
                modified: Some(meta.last_modified.timestamp()),
            });
        }
        Ok(out)
    }

    async fn download(
        &self,
        remote: &str,
        local: &Path,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let get = self
            .store
            .get(&key(remote))
            .await
            .map_err(|e| map_obj(remote, e))?;
        let total = Some(get.meta.size);
        let stream = get.into_stream().map_err(std::io::Error::other);
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
        let mut dst = BufWriter::new(self.store.clone(), key(remote));
        let result = copy_with_progress(src, &mut dst, total, ctl).await;
        match result {
            Ok(r) => {
                dst.shutdown().await.map_err(|e| FpError::Protocol(e.to_string()))?;
                Ok(r)
            }
            Err(e) => {
                let _ = dst.abort().await;
                Err(e)
            }
        }
    }

    async fn mkdir(&self, path: &str) -> Result<(), FpError> {
        // Objektspeicher kennen keine Verzeichnisse — ein leeres Marker-Objekt
        // im Ordner macht das Präfix sichtbar. (Ein Schlüssel mit Slash am
        // Ende geht nicht: object_store normalisiert Pfade.) Listings blenden
        // den Marker aus; remove(dir) räumt ihn mit ab.
        let marker = ObjPath::from(format!("{}/{DIR_MARKER}", path.trim_matches('/')));
        self.store
            .put(&marker, object_store::PutPayload::new())
            .await
            .map(|_| ())
            .map_err(|e| map_obj(path, e))
    }

    async fn remove(&self, path: &str, is_dir: bool) -> Result<(), FpError> {
        if is_dir {
            // Rekursiv: alle Objekte unter dem Präfix löschen (inkl. Marker).
            let prefix = ObjPath::from(path.trim_matches('/'));
            let locations: Vec<ObjPath> = self
                .store
                .list(Some(&prefix))
                .map_ok(|m| m.location)
                .try_collect()
                .await
                .map_err(|e| map_obj(path, e))?;
            for loc in locations {
                self.store.delete(&loc).await.map_err(|e| map_obj(path, e))?;
            }
            Ok(())
        } else {
            self.store.delete(&key(path)).await.map_err(|e| map_obj(path, e))
        }
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FpError> {
        self.store
            .rename(&key(from), &key(to))
            .await
            .map_err(|e| map_obj(from, e))
    }
}
