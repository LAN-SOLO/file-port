//! Integrationstest gegen einen echten SFTP-Server (Docker: atmoz/sftp).
//!
//! Läuft nur, wenn `FILEPORT_IT=1` gesetzt ist und ein Server auf
//! localhost:2222 lauscht (Nutzer foo/pass, beschreibbares /upload):
//!
//! ```sh
//! docker run --rm -d -p 2222:22 --name fileport-sftp atmoz/sftp foo:pass:::upload
//! FILEPORT_IT=1 cargo test -p fileport-core --test sftp_it
//! docker stop fileport-sftp
//! ```

use fileport_core::{Backend, SftpAuth, SftpBackend, SftpConfig, TransferCtl};

fn config(expected_host_key: Option<String>) -> SftpConfig {
    SftpConfig {
        host: "127.0.0.1".into(),
        port: 2222,
        user: "foo".into(),
        auth: SftpAuth::Password("pass".into()),
        expected_host_key,
    }
}

#[tokio::test]
async fn sftp_roundtrip_against_real_server() {
    if std::env::var("FILEPORT_IT").as_deref() != Ok("1") {
        eprintln!("übersprungen — FILEPORT_IT=1 setzen und Docker-SFTP starten");
        return;
    }

    let be = SftpBackend::connect(config(None)).await.unwrap();
    let host_key = be.host_key().to_string();
    assert!(!host_key.is_empty(), "TOFU muss den Host-Key liefern");

    // Wiederverbinden mit gespeichertem Key muss klappen …
    let be = SftpBackend::connect(config(Some(host_key.clone()))).await.unwrap();
    // … und mit falschem Key scheitern.
    let bogus = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIP///////////////////////////////////w bogus";
    assert!(SftpBackend::connect(config(Some(bogus.into()))).await.is_err());

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 241) as u8).collect();
    tokio::fs::write(&src, &data).await.unwrap();

    be.mkdir("/upload/it").await.unwrap();
    let up = be.upload(&src, "/upload/it/file.bin", &TransferCtl::noop()).await.unwrap();
    assert_eq!(up.bytes, data.len() as u64);

    let listing = be.list("/upload/it").await.unwrap();
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "file.bin");
    assert_eq!(listing[0].size, data.len() as u64);

    let dst = dir.path().join("dst.bin");
    let down = be.download("/upload/it/file.bin", &dst, &TransferCtl::noop()).await.unwrap();
    assert_eq!(down.blake3, up.blake3);
    assert_eq!(tokio::fs::read(&dst).await.unwrap(), data);

    be.rename("/upload/it/file.bin", "/upload/it/renamed.bin").await.unwrap();
    be.remove("/upload/it/renamed.bin", false).await.unwrap();
    be.remove("/upload/it", true).await.unwrap();
    assert!(be.list("/upload/it").await.is_err());
}
