//! Integrationstest gegen einen echten SMB-Server (Docker: dperson/samba).
//!
//! ```sh
//! docker run --rm -d --name fileport-smb -p 4450:445 \
//!   dperson/samba -u "foo;pass" -s "testshare;/share;no;no;no;foo" -p
//! FILEPORT_IT=1 cargo test -p fileport-core --test smb_it
//! docker stop fileport-smb
//! ```

use fileport_core::{Backend, EntryKind, SmbBackend, SmbConfig, TransferCtl};

fn config(password: &str) -> SmbConfig {
    SmbConfig {
        host: "127.0.0.1".into(),
        port: 4450,
        share: "testshare".into(),
        user: "foo".into(),
        password: password.into(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn smb_roundtrip_against_real_server() {
    if std::env::var("FILEPORT_IT").as_deref() != Ok("1") {
        eprintln!("übersprungen — FILEPORT_IT=1 setzen und Docker-Samba starten");
        return;
    }

    // Falsche Zugangsdaten müssen schon beim Verbinden scheitern.
    assert!(SmbBackend::connect(config("falsch")).await.is_err());

    let be = SmbBackend::connect(config("pass")).await.unwrap();
    assert_eq!(be.initial_dir().await.unwrap(), "/");

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    tokio::fs::write(&src, &data).await.unwrap();

    be.mkdir("/it test").await.unwrap();
    let up = be
        .upload(&src, "/it test/file ä.bin", &TransferCtl::noop())
        .await
        .unwrap();
    assert_eq!(up.bytes, data.len() as u64);

    let root = be.list("/").await.unwrap();
    assert!(root.iter().any(|e| e.name == "it test" && e.kind == EntryKind::Dir));
    let listing = be.list("/it test").await.unwrap();
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "file ä.bin");
    assert_eq!(listing[0].size, data.len() as u64);

    let dst = dir.path().join("dst.bin");
    let down = be
        .download("/it test/file ä.bin", &dst, &TransferCtl::noop())
        .await
        .unwrap();
    assert_eq!(down.blake3, up.blake3);
    assert_eq!(tokio::fs::read(&dst).await.unwrap(), data);

    be.rename("/it test/file ä.bin", "/it test/renamed.bin")
        .await
        .unwrap();
    be.remove("/it test", true).await.unwrap();
    assert!(be.list("/it test").await.is_err());
}
