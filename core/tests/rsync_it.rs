//! Integrationstest für rsync-über-SSH gegen einen echten SSH-Server
//! (Docker: linuxserver/openssh-server + rsync). Braucht Schlüssel-Auth,
//! weil die Transfers das lokale rsync/ssh im Batch-Mode spawnen.
//!
//! ```sh
//! ssh-keygen -t ed25519 -N "" -f /tmp/fileport-it-key
//! docker run --rm -d --name fileport-rsync -p 2224:2222 \
//!   -e PUID=1000 -e PGID=1000 -e USER_NAME=foo \
//!   -e PUBLIC_KEY="$(cat /tmp/fileport-it-key.pub)" \
//!   lscr.io/linuxserver/openssh-server
//! docker exec fileport-rsync apk add --no-cache rsync
//! FILEPORT_IT=1 FILEPORT_IT_SSH_KEY=/tmp/fileport-it-key \
//!   cargo test -p fileport-core --test rsync_it
//! docker stop fileport-rsync
//! ```

use fileport_core::{Backend, RsyncBackend, SshAuth, SshConfig, TransferCtl};

#[tokio::test(flavor = "multi_thread")]
async fn rsync_roundtrip_against_real_server() {
    if std::env::var("FILEPORT_IT").as_deref() != Ok("1") {
        eprintln!("übersprungen — FILEPORT_IT=1 setzen und Docker-SSH starten");
        return;
    }
    let Ok(key) = std::env::var("FILEPORT_IT_SSH_KEY") else {
        eprintln!("übersprungen — FILEPORT_IT_SSH_KEY auf die Schlüsseldatei setzen");
        return;
    };

    // Eigenes HOME, damit das gespawnte ssh (accept-new) seine known_hosts
    // nicht in die des Entwicklers schreibt.
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".ssh")).unwrap();
    std::env::set_var("HOME", home.path());

    let be = RsyncBackend::connect(SshConfig {
        host: "127.0.0.1".into(),
        port: 2224,
        user: "foo".into(),
        auth: SshAuth::KeyFile {
            path: key.clone().into(),
            passphrase: None,
        },
        expected_host_key: None,
    })
    .await
    .unwrap();

    let home_dir = be.initial_dir().await.unwrap();
    assert!(home_dir.starts_with('/'));

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 199) as u8).collect();
    tokio::fs::write(&src, &data).await.unwrap();

    let base = format!("{}/it test", home_dir.trim_end_matches('/'));
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

    // Passwort-Auth muss rsync klar ablehnen (Transfers wären unmöglich).
    let pw = RsyncBackend::connect(SshConfig {
        host: "127.0.0.1".into(),
        port: 2224,
        user: "foo".into(),
        auth: SshAuth::Password("egal".into()),
        expected_host_key: None,
    })
    .await;
    assert!(matches!(pw, Err(fileport_core::FpError::Auth(_))));
}
