//! Integrationstest gegen einen echten FTP-Server (Docker: delfer/alpine-ftp-server).
//!
//! ```sh
//! docker run --rm -d --name fileport-ftp -p 2121:21 -p 21000-21010:21000-21010 \
//!   -e USERS="foo|pass" -e ADDRESS=127.0.0.1 delfer/alpine-ftp-server
//! FILEPORT_IT=1 cargo test -p fileport-core --test ftp_it
//! docker stop fileport-ftp
//! ```

use fileport_core::{Backend, FtpBackend, FtpConfig, FtpSecurity, TransferCtl};

#[tokio::test(flavor = "multi_thread")]
async fn ftp_roundtrip_against_real_server() {
    if std::env::var("FILEPORT_IT").as_deref() != Ok("1") {
        eprintln!("übersprungen — FILEPORT_IT=1 setzen und Docker-FTP starten");
        return;
    }

    let be = FtpBackend::connect(FtpConfig {
        host: "127.0.0.1".into(),
        port: 2121,
        user: "foo".into(),
        password: "pass".into(),
        security: FtpSecurity::Plain,
        accept_invalid_certs: false,
    })
    .await
    .unwrap();

    let home = be.initial_dir().await.unwrap();
    assert!(home.starts_with('/'));

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 239) as u8).collect();
    tokio::fs::write(&src, &data).await.unwrap();

    let base = format!("{}/it", home.trim_end_matches('/'));
    be.mkdir(&base).await.unwrap();
    let up = be
        .upload(&src, &format!("{base}/file.bin"), &TransferCtl::noop())
        .await
        .unwrap();
    assert_eq!(up.bytes, data.len() as u64);

    let listing = be.list(&base).await.unwrap();
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "file.bin");
    assert_eq!(listing[0].size, data.len() as u64);

    let dst = dir.path().join("dst.bin");
    let down = be
        .download(&format!("{base}/file.bin"), &dst, &TransferCtl::noop())
        .await
        .unwrap();
    assert_eq!(down.blake3, up.blake3);
    assert_eq!(tokio::fs::read(&dst).await.unwrap(), data);

    be.rename(&format!("{base}/file.bin"), &format!("{base}/renamed.bin"))
        .await
        .unwrap();
    be.remove(&format!("{base}/renamed.bin"), false).await.unwrap();
    be.remove(&base, true).await.unwrap();

    // Falsche Zugangsdaten müssen als Auth-Fehler ankommen.
    let bad = FtpBackend::connect(FtpConfig {
        host: "127.0.0.1".into(),
        port: 2121,
        user: "foo".into(),
        password: "falsch".into(),
        security: FtpSecurity::Plain,
        accept_invalid_certs: false,
    })
    .await;
    assert!(matches!(bad, Err(fileport_core::FpError::Auth(_))));
}
