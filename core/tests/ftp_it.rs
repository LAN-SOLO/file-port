//! Integrationstest gegen einen echten FTPS-Server (Docker: vsftpd aus
//! delfer/alpine-ftp-server mit erzwungenem TLS und selbstsigniertem
//! Zertifikat). vsftpd verlangt standardmäßig TLS-Session-Wiederaufnahme
//! auf dem Datenkanal (`require_ssl_reuse=YES`) — genau das prüft der Test
//! mit. Klartext-FTP unterstützt fileport seit 0.2.8 nicht mehr.
//!
//! ```sh
//! mkdir -p /tmp/fileport-ftps
//! openssl req -x509 -nodes -newkey rsa:2048 -days 2 -subj "/CN=127.0.0.1" \
//!   -keyout /tmp/fileport-ftps/ftp.key -out /tmp/fileport-ftps/ftp.crt
//! docker run --rm -d --name fileport-ftps -p 2121:21 -p 21000-21010:21000-21010 \
//!   -e USERS="foo|pass" -e ADDRESS=127.0.0.1 \
//!   -e TLS_CERT=/certs/ftp.crt -e TLS_KEY=/certs/ftp.key \
//!   -v /tmp/fileport-ftps:/certs delfer/alpine-ftp-server
//! FILEPORT_IT=1 cargo test -p fileport-core --test ftp_it
//! docker stop fileport-ftps
//! ```
//!
//! vsftpd kennt kein MLSD (fileport fällt auf LIST zurück). Den MLSD-Pfad
//! (ProFTPD/FileZilla Server/Pure-FTPd) prüft derselbe Test gegen
//! pyftpdlib mit TLS auf Port 2121 (`TLS_FTPHandler`, Benutzer foo/pass,
//! passive Ports 21000–21010, `tls_control_required`/`tls_data_required`).

use fileport_core::{Backend, FtpBackend, FtpConfig, FtpSecurity, TransferCtl};

#[tokio::test(flavor = "multi_thread")]
async fn ftps_roundtrip_against_real_server() {
    if std::env::var("FILEPORT_IT").as_deref() != Ok("1") {
        eprintln!("übersprungen — FILEPORT_IT=1 setzen und Docker-FTP starten");
        return;
    }

    let be = FtpBackend::connect(FtpConfig {
        host: "127.0.0.1".into(),
        port: 2121,
        user: "foo".into(),
        password: "pass".into(),
        security: FtpSecurity::ExplicitTls,
        accept_invalid_certs: true,
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
    // Änderungszeit: vsftpd per LIST (UTC-Annahme), Server mit MLSD per UTC-Fakt —
    // beides muss nahe „jetzt“ liegen, nicht um den Zeitzonen-Versatz verschoben.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mtime = listing[0].modified.expect("Änderungszeit fehlt");
    assert!((mtime - now).abs() < 600, "mtime {mtime} vs. jetzt {now}");

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

    // Nicht-leeren Ordner rekursiv löschen (RMD allein kann das nicht)
    be.mkdir(&format!("{base}/sub")).await.unwrap();
    be.upload(&src, &format!("{base}/sub/inner.bin"), &TransferCtl::noop())
        .await
        .unwrap();
    be.remove(&base, true).await.unwrap();
    assert!(!be
        .list(&home)
        .await
        .unwrap()
        .iter()
        .any(|e| e.name == "it"));

    // Falsche Zugangsdaten müssen als Auth-Fehler ankommen.
    let bad = FtpBackend::connect(FtpConfig {
        host: "127.0.0.1".into(),
        port: 2121,
        user: "foo".into(),
        password: "falsch".into(),
        security: FtpSecurity::ExplicitTls,
        accept_invalid_certs: true,
    })
    .await;
    assert!(matches!(bad, Err(fileport_core::FpError::Auth(_))));
}

/// Implizites FTPS (TLS ab dem ersten Byte). Zweiter vsftpd mit
/// `implicit_ssl=YES` auf Port 9990:
///
/// ```sh
/// docker run --rm -d --name fileport-ftps-implicit -p 9990:990 -p 21100-21110:21100-21110 \
///   -e USERS="foo|pass" -v /tmp/fileport-ftps:/certs delfer/alpine-ftp-server \
///   sh -c 'vsftpd -olisten_port=990 -oimplicit_ssl=YES -opasv_min_port=21100 \
///     -opasv_max_port=21110 -opasv_address=127.0.0.1 -orsa_cert_file=/certs/ftp.crt \
///     -orsa_private_key_file=/certs/ftp.key -ossl_enable=YES -oforce_local_data_ssl=YES \
///     -oforce_local_logins_ssl=YES /etc/vsftpd/vsftpd.conf; sleep infinity'
/// FILEPORT_IT=1 FILEPORT_IT_FTPS_IMPLICIT=1 cargo test -p fileport-core --test ftp_it
/// ```
#[tokio::test]
async fn ftps_implicit_against_real_server() {
    if std::env::var("FILEPORT_IT").as_deref() != Ok("1")
        || std::env::var("FILEPORT_IT_FTPS_IMPLICIT").as_deref() != Ok("1")
    {
        eprintln!("übersprungen — FILEPORT_IT=1 und FILEPORT_IT_FTPS_IMPLICIT=1 setzen");
        return;
    }
    let be = FtpBackend::connect(FtpConfig {
        host: "127.0.0.1".into(),
        port: 9990,
        user: "foo".into(),
        password: "pass".into(),
        security: FtpSecurity::ImplicitTls,
        accept_invalid_certs: true,
    })
    .await
    .unwrap();
    let home = be.initial_dir().await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    tokio::fs::write(&src, vec![7u8; 70_000]).await.unwrap();
    let target = format!("{}/implicit.bin", home.trim_end_matches('/'));
    let up = be.upload(&src, &target, &TransferCtl::noop()).await.unwrap();
    let down = be
        .download(&target, &dir.path().join("dst.bin"), &TransferCtl::noop())
        .await
        .unwrap();
    assert_eq!(up.blake3, down.blake3);
    be.remove(&target, false).await.unwrap();

    // Ohne „selbstsignierte akzeptieren" muss das Testzertifikat abgelehnt werden.
    let strict = FtpBackend::connect(FtpConfig {
        host: "127.0.0.1".into(),
        port: 9990,
        user: "foo".into(),
        password: "pass".into(),
        security: FtpSecurity::ImplicitTls,
        accept_invalid_certs: false,
    })
    .await;
    assert!(strict.is_err());
}
