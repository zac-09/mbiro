//! The app's calls to the licence service.
//!
//! The ordinary tests need nothing but this machine. The last one runs against
//! the real service from `services/licence`, started by
//! `scripts/test-licence-service.sh`, which is how to run it.

use mbiro_desktop::activation::{self, ActivationError};
use mbiro_desktop::licence;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const KEY: &str = "cGF5bG9hZA.c2lnbmF0dXJl";

fn device(n: u8) -> String {
    format!("{n:064x}")
}

/// Answers every request with the same status line and body.
async fn canned(status: &'static str, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let server = format!("http://{}", listener.local_addr().expect("address"));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request).await;
                let reply = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes()).await;
            });
        }
    });
    server
}

#[tokio::test]
async fn a_service_that_cannot_be_reached_is_offline() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let server = format!("http://{}", listener.local_addr().expect("address"));
    drop(listener);
    assert_eq!(
        activation::activate(&server, KEY, &device(1)).await,
        Err(ActivationError::Offline)
    );
    assert_eq!(
        activation::deactivate(&server, KEY, &device(1)).await,
        Err(ActivationError::Offline)
    );
    assert_eq!(
        activation::revoked(&server).await,
        Err(ActivationError::Offline)
    );
}

#[tokio::test]
async fn each_refusal_is_told_apart() {
    for (status, body, expected) in [
        (
            "403 Forbidden",
            r#"{"ok":false,"code":"invalid_key"}"#,
            ActivationError::InvalidKey,
        ),
        (
            "403 Forbidden",
            r#"{"ok":false,"code":"revoked"}"#,
            ActivationError::Revoked,
        ),
        (
            "409 Conflict",
            r#"{"ok":false,"code":"device_limit"}"#,
            ActivationError::DeviceLimit,
        ),
        (
            "500 Internal Server Error",
            r#"{"ok":false,"code":"failed"}"#,
            ActivationError::Service,
        ),
        // Something that is not the service at all, such as a Wi-Fi sign-in page.
        ("200 OK", "<html>Sign in</html>", ActivationError::Service),
        ("200 OK", r#"{"ok":true}"#, ActivationError::Service),
    ] {
        let server = canned(status, body).await;
        assert_eq!(
            activation::activate(&server, KEY, &device(1)).await,
            Err(expected),
            "{status} {body}"
        );
    }
}

#[tokio::test]
async fn only_well_formed_ids_are_kept_from_the_refund_list() {
    let server = canned(
        "200 OK",
        r#"{"revoked":["00000000000000000000000000000000000000000000000000000000000000aa","junk",""]}"#,
    )
    .await;
    assert_eq!(
        activation::revoked(&server).await,
        Ok(vec![format!("{:064x}", 0xaa)])
    );
    let broken = canned("200 OK", "not json").await;
    assert_eq!(
        activation::revoked(&broken).await,
        Err(ActivationError::Service)
    );
}

/// Against the real service. Needs, from `scripts/test-licence-service.sh`:
/// MBIRO_TEST_LICENCE_SERVER, MBIRO_TEST_PUBLIC_KEY, MBIRO_TEST_LICENCE (a key
/// it issued) and MBIRO_TEST_REFUNDED_LICENCE (one it issued, then refunded).
#[tokio::test]
#[ignore = "needs the licence service running: scripts/test-licence-service.sh"]
async fn agrees_with_the_real_service() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let server = var("MBIRO_TEST_LICENCE_SERVER");
    let public = var("MBIRO_TEST_PUBLIC_KEY");
    let key = var("MBIRO_TEST_LICENCE");
    let refunded = var("MBIRO_TEST_REFUNDED_LICENCE");

    // The app and the service read the same key the same way.
    let said = licence::verify(&key, &public).expect("the app accepts the service's key");
    assert!(said.email.contains('@'));

    // Three Macs, each with a receipt that is good for that Mac only.
    for n in 1..=3 {
        let receipt = activation::activate(&server, &key, &device(n))
            .await
            .expect("a place");
        assert!(licence::receipt_covers(&receipt, &public, &key, &device(n)));
        assert!(!licence::receipt_covers(
            &receipt,
            &public,
            &key,
            &device(9)
        ));
        assert!(!licence::receipt_covers(
            &receipt,
            &public,
            &refunded,
            &device(n)
        ));
    }
    assert_eq!(
        activation::activate(&server, &key, &device(4)).await,
        Err(ActivationError::DeviceLimit)
    );
    // Removing the licence from one Mac makes room for another.
    activation::deactivate(&server, &key, &device(1))
        .await
        .expect("a place freed");
    assert!(activation::activate(&server, &key, &device(4))
        .await
        .is_ok());

    assert_eq!(
        activation::activate(&server, KEY, &device(1)).await,
        Err(ActivationError::InvalidKey)
    );
    assert_eq!(
        activation::activate(&server, &refunded, &device(1)).await,
        Err(ActivationError::Revoked)
    );
    let revoked = activation::revoked(&server).await.expect("the list");
    assert_eq!(revoked, vec![licence::key_id(&refunded)]);
}
