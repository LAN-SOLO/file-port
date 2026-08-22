//! Integrationstest gegen einen echten SSH-Server mit Shell-Zugang
//! (Docker: linuxserver/openssh-server) — atmoz/sftp taugt hier nicht,
//! weil er per ForceCommand nur das SFTP-Subsystem erlaubt und SCP
//! Shell-Kommandos braucht.
//!
//! ```sh
//! docker run --rm -d --name fileport-scp -p 2223:2222 \
//!   -e PUID=1000 -e PGID=1000 -e PASSWORD_ACCESS=true \
//!   -e USER_NAME=foo -e USER_PASSWORD=pass -e SUDO_ACCESS=false \
//!   lscr.io/linuxserver/openssh-server
//! FILEPORT_IT=1 cargo test -p fileport-core --test scp_it
//! docker stop fileport-scp
//! ```

use fileport_core::{Backend, ScpBackend, SshAuth, SshConfig, TransferCtl};

fn config() -> SshConfig {
    SshConfig {
        host: "127.0.0.1".into(),
        port: 2223,
        user: "foo".into(),
        auth: SshAuth::Password("pass".into()),
        expected_host_key: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn scp_roundtrip_against_real_server() {
    if std::env::var("FILEPORT_IT").as_deref() != Ok("1") {
        eprintln!("übersprungen — FILEPORT_IT=1 setzen und Docker-SSH starten");
        return;
    }

    let be = ScpBackend::connect(config()).await.unwrap();
    let home = be.initial_dir().await.unwrap();
    assert!(home.starts_with('/'));

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 241) as u8).collect();
    tokio::fs::write(&src, &data).await.unwrap();

    let base = format!("{}/it test", home.trim_end_matches('/'));
    be.mkdir(&base).await.unwrap();
    let up = be
        .upload(&src, &format!("{base}/file ä.bin"), &TransferCtl::noop())
        .await
        .unwrap();
    assert_eq!(up.bytes, data.len() as u64);

    let listing = be.list(&base).await.unwrap();
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "file ä.bin");
    assert_eq!(listing[0].size, data.len() as u64);

    let dst = dir.path().join("dst.bin");
    let down = be
        .download(&format!("{base}/file ä.bin"), &dst, &TransferCtl::noop())
        .await
        .unwrap();
    assert_eq!(down.blake3, up.blake3);
    assert_eq!(tokio::fs::read(&dst).await.unwrap(), data);

    be.rename(&format!("{base}/file ä.bin"), &format!("{base}/renamed.bin"))
        .await
        .unwrap();
    be.remove(&format!("{base}/renamed.bin"), false).await.unwrap();
    be.remove(&base, true).await.unwrap();
    assert!(be.list(&base).await.is_err());

    // Falsche Zugangsdaten müssen als Auth-Fehler ankommen.
    let bad = ScpBackend::connect(SshConfig {
        auth: SshAuth::Password("falsch".into()),
        ..config()
    })
    .await;
    assert!(matches!(bad, Err(fileport_core::FpError::Auth(_))));
}
