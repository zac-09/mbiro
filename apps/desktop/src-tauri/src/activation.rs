//! The app's side of the licence service (`services/licence`): taking one of a
//! key's three places for this Mac, giving it back, and fetching the list of
//! refunded keys.
//!
//! These are the only calls the app makes about a licence. They carry the key
//! and a hash that stands for this Mac, and nothing else: no name, no serial
//! number, no account. Nothing here is logged.

use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::constants::BUNDLE_IDENTIFIER;

/// Where the licence service lives, as `https://host` with no trailing slash.
/// `None` until it is deployed. Until then keys are checked on this Mac only:
/// nothing is sent, and the three-Mac limit and refunds are not enforced.
pub const SERVER: Option<&str> = None;

/// In development builds only, names a service to use instead, such as the
/// local one: `MBIRO_LICENCE_SERVER=http://127.0.0.1:8787 pnpm dev`.
#[cfg(debug_assertions)]
const SERVER_OVERRIDE: &str = "MBIRO_LICENCE_SERVER";

/// The service this run talks to, if any.
pub fn server() -> Option<String> {
    #[cfg(debug_assertions)]
    if let Some(url) = std::env::var(SERVER_OVERRIDE)
        .ok()
        .filter(|url| !url.is_empty())
    {
        return Some(url.trim_end_matches('/').to_owned());
    }
    SERVER.map(str::to_owned)
}

#[derive(Debug, PartialEq)]
pub enum ActivationError {
    /// The service could not be reached, or did not answer in time.
    Offline,
    /// The service does not accept the key as one of ours.
    InvalidKey,
    /// The key was refunded.
    Revoked,
    /// The key's places are all taken by other Macs.
    DeviceLimit,
    /// The service answered, but not with anything we understand.
    Service,
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    ok: bool,
    code: Option<String>,
    receipt: Option<String>,
}

#[derive(Deserialize)]
struct RevokedReply {
    revoked: Vec<String>,
}

fn client() -> Result<reqwest::Client, ActivationError> {
    reqwest::Client::builder()
        // A release build talks to the service over HTTPS or not at all.
        .https_only(!cfg!(debug_assertions))
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ActivationError::Service)
}

async fn post(server: &str, path: &str, key: &str, device: &str) -> Result<Reply, ActivationError> {
    let response = client()?
        .post(format!("{server}{path}"))
        .json(&serde_json::json!({ "key": key, "device": device }))
        .send()
        .await
        .map_err(|_| ActivationError::Offline)?;
    let reply: Reply = response
        .json()
        .await
        .map_err(|_| ActivationError::Service)?;
    if reply.ok {
        return Ok(reply);
    }
    Err(match reply.code.as_deref() {
        Some("invalid_key") => ActivationError::InvalidKey,
        Some("revoked") => ActivationError::Revoked,
        Some("device_limit") => ActivationError::DeviceLimit,
        _ => ActivationError::Service,
    })
}

/// Takes a place on the key for this Mac and returns the service's receipt.
/// The caller checks the receipt before trusting it.
pub async fn activate(server: &str, key: &str, device: &str) -> Result<String, ActivationError> {
    post(server, "/v1/activate", key, device)
        .await?
        .receipt
        .ok_or(ActivationError::Service)
}

/// Gives this Mac's place on the key back.
pub async fn deactivate(server: &str, key: &str, device: &str) -> Result<(), ActivationError> {
    post(server, "/v1/deactivate", key, device).await.map(drop)
}

/// The ids of every refunded licence. Sends nothing about this Mac or its key.
pub async fn revoked(server: &str) -> Result<Vec<String>, ActivationError> {
    let response = client()?
        .get(format!("{server}/v1/revoked"))
        .send()
        .await
        .map_err(|_| ActivationError::Offline)?;
    if !response.status().is_success() {
        return Err(ActivationError::Service);
    }
    let reply: RevokedReply = response
        .json()
        .await
        .map_err(|_| ActivationError::Service)?;
    // Only well-formed ids are kept: this list is stored and compared, never shown.
    Ok(reply
        .revoked
        .into_iter()
        .filter(|id| id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()))
        .take(100_000)
        .collect())
}

/// What stands for this Mac at the licence service: a hash of the hardware
/// identifier, salted with the app's own name so it means nothing elsewhere.
/// `None` if the identifier cannot be read.
pub fn device_id() -> Option<String> {
    let output = std::process::Command::new("/usr/sbin/ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    hardware_uuid(&text).map(|uuid| device_hash(&uuid))
}

/// Picks `"IOPlatformUUID" = "..."` out of what `ioreg` prints.
fn hardware_uuid(ioreg: &str) -> Option<String> {
    let line = ioreg.lines().find(|line| line.contains("IOPlatformUUID"))?;
    let uuid = line.split('"').nth(3)?;
    (uuid.len() >= 16).then(|| uuid.to_owned())
}

fn device_hash(uuid: &str) -> String {
    Sha256::digest(format!("{BUNDLE_IDENTIFIER}:{uuid}").as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_hardware_identifier_and_never_passes_it_on() {
        let printed = r#"
+-o J314sAP  <class IOPlatformExpertDevice, id 0x100000254>
    {
      "IOPlatformSerialNumber" = "XXXXXXXXXX"
      "IOPlatformUUID" = "01234567-89AB-CDEF-0123-456789ABCDEF"
    }"#;
        let uuid = hardware_uuid(printed).expect("the identifier");
        assert_eq!(uuid, "01234567-89AB-CDEF-0123-456789ABCDEF");
        let id = device_hash(&uuid);
        assert_eq!(id.len(), 64);
        assert!(id.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(!id.contains("0123456789ab"));
        assert_eq!(hardware_uuid("nothing useful"), None);
    }

    #[test]
    fn this_mac_has_a_stable_id() {
        let first = device_id().expect("ioreg names this Mac");
        assert_eq!(first.len(), 64);
        assert_eq!(device_id(), Some(first));
    }
}
