//! Integrationstest gegen einen echten WebDAV-Server (Docker: bytemark/webdav).
//!
//! ```sh
//! docker run --rm -d --name fileport-dav -p 8081:80 \
//!   -e AUTH_TYPE=Basic -e USERNAME=foo -e PASSWORD=pass bytemark/webdav
//! FILEPORT_IT=1 cargo test -p fileport-core --test webdav_it
//! docker stop fileport-dav
//! ```

use fileport_core::{Backend, EntryKind, TransferCtl, WebdavBackend, WebdavConfig};

fn config(user: &str, password: &str) -> WebdavConfig {
    WebdavConfig {
        base_url: "http://127.0.0.1:8081".into(),
        user: user.into(),
        password: password.into(),
        accept_invalid_certs: false,
    }
}

#[tokio::test]
async fn webdav_roundtrip_against_real_server() {
    if std::env::var("FILEPORT_IT").as_deref() != Ok("1") {
        eprintln!("übersprungen — FILEPORT_IT=1 setzen und Docker-WebDAV starten");
        return;
    }

    // Falsche Zugangsdaten müssen schon beim Verbinden scheitern.
    assert!(WebdavBackend::connect(config("foo", "falsch")).await.is_err());

    let be = WebdavBackend::connect(config("foo", "pass")).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 233) as u8).collect();
    tokio::fs::write(&src, &data).await.unwrap();

    be.mkdir("/it test").await.unwrap();
    let up = be
        .upload(&src, "/it test/file ä.bin", &TransferCtl::noop())
        .await
        .unwrap();
    assert_eq!(up.bytes, data.len() as u64);

    let listing = be.list("/it test").await.unwrap();
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "file ä.bin");
    assert_eq!(listing[0].kind, EntryKind::File);
    assert_eq!(listing[0].size, data.len() as u64);

    let dst = dir.path().join("dst.bin");
    let down = be
        .download("/it test/file ä.bin", &dst, &TransferCtl::noop())
        .await
        .unwrap();
    assert_eq!(down.blake3, up.blake3);
    assert_eq!(tokio::fs::read(&dst).await.unwrap(), data);

    be.rename("/it test/file ä.bin", "/it test/renamed.bin").await.unwrap();
    be.remove("/it test/renamed.bin", false).await.unwrap();
    be.remove("/it test", true).await.unwrap();
    assert!(be.list("/it test").await.is_err());
}
