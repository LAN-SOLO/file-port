//! Integrationstest gegen einen echten S3-kompatiblen Server (Docker: MinIO).
//!
//! MinIO-Images sind seit 2025 nicht mehr frei auf Docker Hub — der Test
//! läuft gegen `rclone serve s3` (Bucket = Unterordner des Datenverzeichnisses):
//!
//! ```sh
//! mkdir -p /tmp/fileport-s3/testbucket
//! docker run --rm -d --name fileport-s3 -p 9000:8080 -v /tmp/fileport-s3:/data \
//!   rclone/rclone serve s3 --addr :8080 --auth-key fileport,fileport-secret /data
//! FILEPORT_IT=1 cargo test -p fileport-core --test s3_it
//! docker stop fileport-s3
//! ```

use fileport_core::{Backend, EntryKind, ObjectBackend, S3Config, TransferCtl};

#[tokio::test]
async fn s3_roundtrip_against_real_server() {
    if std::env::var("FILEPORT_IT").as_deref() != Ok("1") {
        eprintln!("übersprungen — FILEPORT_IT=1 setzen und Docker-MinIO starten");
        return;
    }

    let config = |secret: &str| S3Config {
        endpoint: Some("http://127.0.0.1:9000".into()),
        region: "us-east-1".into(),
        bucket: "testbucket".into(),
        access_key: "fileport".into(),
        secret_key: secret.into(),
        path_style: true,
        accept_invalid_certs: false,
    };

    // Falsche Zugangsdaten müssen schon beim Verbinden scheitern.
    assert!(ObjectBackend::connect_s3(config("falsch-falsch")).await.is_err());

    let be = ObjectBackend::connect_s3(config("fileport-secret")).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 229) as u8).collect();
    tokio::fs::write(&src, &data).await.unwrap();

    be.mkdir("/it").await.unwrap();
    let up = be.upload(&src, "/it/file.bin", &TransferCtl::noop()).await.unwrap();
    assert_eq!(up.bytes, data.len() as u64);

    let root = be.list("/").await.unwrap();
    assert!(root.iter().any(|e| e.name == "it" && e.kind == EntryKind::Dir));

    let listing = be.list("/it").await.unwrap();
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "file.bin");
    assert_eq!(listing[0].size, data.len() as u64);

    let dst = dir.path().join("dst.bin");
    let down = be.download("/it/file.bin", &dst, &TransferCtl::noop()).await.unwrap();
    assert_eq!(down.blake3, up.blake3);
    assert_eq!(tokio::fs::read(&dst).await.unwrap(), data);

    be.rename("/it/file.bin", "/it/renamed.bin").await.unwrap();
    let listing = be.list("/it").await.unwrap();
    assert_eq!(listing[0].name, "renamed.bin");

    // Ordner umbenennen: alle Objekte unter dem Präfix wandern mit
    be.rename("/it", "/it2").await.unwrap();
    assert!(be.list("/it").await.unwrap().is_empty());
    assert_eq!(be.list("/it2").await.unwrap()[0].name, "renamed.bin");

    be.remove("/it2", true).await.unwrap();
    let root = be.list("/").await.unwrap();
    assert!(!root.iter().any(|e| e.name == "it" || e.name == "it2"));
}
