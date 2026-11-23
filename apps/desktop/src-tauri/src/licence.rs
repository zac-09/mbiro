//! The trial and the licence key, both worked out on this Mac.
//!
//! A licence key is a small JSON payload and an Ed25519 signature of it, each
//! in URL-safe base64, joined by a dot. The app holds only the public key: it
//! can check a key but never make one.
//!
//! The trial is fourteen days from first launch, with everything on. After
//! that, without a licence: one download at a time, two connections, and no
//! video downloads.
//!
//! When the build names a licence server, a key also needs an activation
//! receipt: the server's signed word that this Mac holds one of the key's
//! three places. The receipt and the list of refunded keys are fetched by
//! `activation.rs`; this file only checks them, with no network.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The key licences are checked against, as standard base64 of its 32 bytes.
/// `None` until the owner generates the real pair with
/// `scripts/licence-keygen.sh` and pastes the public half here. Until then no
/// key can be accepted, and the app says so.
pub const PUBLIC_KEY: Option<&str> = None;

pub const TRIAL_DAYS: i64 = 14;
const DAY: i64 = 24 * 60 * 60;

/// What a restricted app may do.
pub const RESTRICTED_MAX_CONCURRENT: usize = 1;
pub const RESTRICTED_CONNECTIONS: u8 = 2;

/// What a licence key says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Licence {
    pub email: String,
    pub plan: String,
    /// The day it was issued, `YYYY-MM-DD`.
    pub issued: String,
}

#[derive(Debug, PartialEq)]
pub enum KeyError {
    /// This build has no public key to check against.
    NoPublicKey,
    /// Not the shape of a key at all.
    Malformed,
    /// The shape of a key, but not one we signed.
    BadSignature,
}

/// Checks `<payload>.<signature>` against a public key and returns the payload.
fn signed_payload(signed: &str, public_key_b64: &str) -> Result<Vec<u8>, KeyError> {
    let (payload, signature) = signed.split_once('.').ok_or(KeyError::Malformed)?;
    let payload = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| KeyError::Malformed)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| KeyError::Malformed)?;
    let signature = Signature::from_slice(&signature).map_err(|_| KeyError::Malformed)?;

    let public = base64::engine::general_purpose::STANDARD
        .decode(public_key_b64)
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
        .ok_or(KeyError::NoPublicKey)?;
    public
        .verify_strict(&payload, &signature)
        .map_err(|_| KeyError::BadSignature)?;
    Ok(payload)
}

/// A key as stored and sent: keys get pasted from emails, so spaces and line
/// breaks are forgiven.
pub fn tidy(key: &str) -> String {
    key.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Checks a key against a public key and returns what it says.
pub fn verify(key: &str, public_key_b64: &str) -> Result<Licence, KeyError> {
    let payload = signed_payload(&tidy(key), public_key_b64)?;
    // Only now is the payload trusted enough to read.
    let licence: Licence = serde_json::from_slice(&payload).map_err(|_| KeyError::Malformed)?;
    let sane = !licence.email.is_empty() && licence.email.len() <= 320 && licence.plan.len() <= 64;
    sane.then_some(licence).ok_or(KeyError::Malformed)
}

/// How a licence is named outside the key itself: in receipts and in the list
/// of refunded keys. The licence service computes the same value
/// (`keyId` in services/licence/src/keys.ts).
pub fn key_id(key: &str) -> String {
    Sha256::digest(tidy(key).as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// What an activation receipt says. Its fields are not a licence's, so one
/// can never pass for the other.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    licence: String,
    device: String,
}

/// Whether `receipt` is the server's word that `key` may run on `device`.
pub fn receipt_covers(receipt: &str, public_key_b64: &str, key: &str, device: &str) -> bool {
    signed_payload(receipt.trim(), public_key_b64)
        .ok()
        .and_then(|payload| serde_json::from_slice::<Receipt>(&payload).ok())
        .is_some_and(|said| said.licence == key_id(key) && said.device == device)
}

/// Where the app stands, as the window shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Status {
    Licensed {
        email: String,
        plan: String,
    },
    Trial {
        #[serde(rename = "daysLeft")]
        days_left: i64,
    },
    /// The trial is over and there is no licence: the limits apply.
    Expired,
}

impl Status {
    pub fn restricted(&self) -> bool {
        *self == Self::Expired
    }
}

/// What is remembered between launches.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Record {
    /// When the trial began, in seconds since 1970.
    pub trial_started: Option<i64>,
    /// The latest time the app has ever seen. Setting the clock back does not
    /// buy more trial: time is taken to be at least this.
    pub last_seen: Option<i64>,
    pub key: Option<String>,
    /// The server's receipt for `key` on this Mac.
    pub receipt: Option<String>,
    /// Ids of refunded licences, as last fetched.
    pub revoked: Vec<String>,
}

/// Whether a key needs the licence server's receipt as well as our signature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Activation<'a> {
    /// This build names no licence server: a valid key is enough.
    NotRequired,
    /// A receipt for this Mac is needed. The id is this Mac's, and is empty
    /// if it could not be read, which no receipt matches.
    Required(&'a str),
}

/// What a stored key is checked against.
#[derive(Debug, Clone, Copy)]
pub struct Check<'a> {
    pub public_key: Option<&'a str>,
    pub activation: Activation<'a>,
}

/// Works out the status at `now`, and what to remember afterwards.
pub fn evaluate(record: &Record, now: i64, check: Check<'_>) -> (Status, Record) {
    let now = now.max(record.last_seen.unwrap_or(0));
    let started = record.trial_started.unwrap_or(now);
    let next = Record {
        trial_started: Some(started),
        last_seen: Some(now),
        ..record.clone()
    };

    let licence = match (&record.key, check.public_key) {
        (Some(key), Some(public)) => verify(key, public).ok().filter(|_| {
            let activated = match check.activation {
                Activation::NotRequired => true,
                Activation::Required(device) => record
                    .receipt
                    .as_deref()
                    .is_some_and(|receipt| receipt_covers(receipt, public, key, device)),
            };
            activated && !record.revoked.contains(&key_id(key))
        }),
        _ => None,
    };
    let status = match licence {
        Some(licence) => Status::Licensed {
            email: licence.email,
            plan: licence.plan,
        },
        None => {
            let left = started + TRIAL_DAYS * DAY - now;
            if left > 0 {
                // Part of a day still counts as a day left.
                Status::Trial {
                    days_left: (left + DAY - 1) / DAY,
                }
            } else {
                Status::Expired
            }
        }
    };
    (status, next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    /// A key pair that exists only in these tests. It signs nothing real.
    fn test_pair() -> (SigningKey, String) {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let public =
            base64::engine::general_purpose::STANDARD.encode(signing.verifying_key().to_bytes());
        (signing, public)
    }

    /// A check with no licence server in the build.
    fn offline(public_key: Option<&str>) -> Check<'_> {
        Check {
            public_key,
            activation: Activation::NotRequired,
        }
    }

    fn issue(signing: &SigningKey, payload: &str) -> String {
        let signature = signing.sign(payload.as_bytes());
        format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(payload),
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        )
    }

    const PAYLOAD: &str =
        r#"{"email":"buyer@example.com","plan":"standard","issued":"2026-11-19"}"#;

    #[test]
    fn accepts_a_key_we_signed_and_reads_it() {
        let (signing, public) = test_pair();
        let key = issue(&signing, PAYLOAD);
        let licence = verify(&key, &public).expect("a valid key");
        assert_eq!(licence.email, "buyer@example.com");
        assert_eq!(licence.plan, "standard");
        assert_eq!(licence.issued, "2026-11-19");
        // As pasted from an email, wrapped and padded with spaces.
        let wrapped = format!("  {}\n{} ", &key[..40], &key[40..]);
        assert!(verify(&wrapped, &public).is_ok());
    }

    #[test]
    fn refuses_anything_else() {
        let (signing, public) = test_pair();
        let key = issue(&signing, PAYLOAD);

        // The payload changed after signing.
        let (_, signature) = key.split_once('.').unwrap();
        let forged = format!(
            "{}.{signature}",
            URL_SAFE_NO_PAD.encode(PAYLOAD.replace("buyer", "thief"))
        );
        assert_eq!(verify(&forged, &public), Err(KeyError::BadSignature));

        // Signed by someone else.
        let other = SigningKey::from_bytes(&[9u8; 32]);
        assert_eq!(
            verify(&issue(&other, PAYLOAD), &public),
            Err(KeyError::BadSignature)
        );

        // Signed by us, but not a licence.
        assert_eq!(
            verify(&issue(&signing, r#"{"email":""}"#), &public),
            Err(KeyError::Malformed)
        );
        assert_eq!(verify("not a key", &public), Err(KeyError::Malformed));
        assert_eq!(verify("", &public), Err(KeyError::Malformed));
        assert_eq!(verify(&key, "not a public key"), Err(KeyError::NoPublicKey));
    }

    /// Checks a key made by `scripts/issue-licence.sh` against the public key
    /// printed by `scripts/licence-keygen.sh`, when both are given:
    ///   MBIRO_TEST_LICENCE=... MBIRO_TEST_PUBLIC_KEY=... cargo test -p mbiro-desktop --lib scripts
    #[test]
    fn accepts_a_key_made_by_the_scripts() {
        let (Ok(key), Ok(public)) = (
            std::env::var("MBIRO_TEST_LICENCE"),
            std::env::var("MBIRO_TEST_PUBLIC_KEY"),
        ) else {
            return; // nothing to check on an ordinary run
        };
        let licence = verify(&key, &public).expect("the scripts and the app must agree");
        assert!(licence.email.contains('@'));
        assert_eq!(licence.issued.len(), 10);
    }

    #[test]
    fn the_trial_counts_down_from_first_launch_and_ends() {
        let start = 1_800_000_000;
        let (status, record) = evaluate(&Record::default(), start, offline(None));
        assert_eq!(status, Status::Trial { days_left: 14 });
        assert_eq!(record.trial_started, Some(start));

        let at = |seconds: i64| evaluate(&record, start + seconds, offline(None)).0;
        assert_eq!(at(DAY - 1), Status::Trial { days_left: 14 });
        assert_eq!(at(DAY), Status::Trial { days_left: 13 });
        assert_eq!(at(13 * DAY + 1), Status::Trial { days_left: 1 });
        assert_eq!(at(14 * DAY), Status::Expired);
        assert!(at(14 * DAY).restricted());
        assert!(!at(0).restricted());
    }

    #[test]
    fn setting_the_clock_back_does_not_extend_the_trial() {
        let start = 1_800_000_000;
        let (_, record) = evaluate(&Record::default(), start, offline(None));
        let (status, record) = evaluate(&record, start + 20 * DAY, offline(None));
        assert_eq!(status, Status::Expired);
        // The clock goes back to the first day; the app remembers better.
        assert_eq!(
            evaluate(&record, start + 60, offline(None)).0,
            Status::Expired
        );
    }

    #[test]
    fn a_valid_key_lifts_the_limits_at_any_time() {
        let (signing, public) = test_pair();
        let start = 1_800_000_000;
        let record = Record {
            trial_started: Some(start),
            last_seen: Some(start),
            key: Some(issue(&signing, PAYLOAD)),
            ..Record::default()
        };
        let (status, _) = evaluate(&record, start + 400 * DAY, offline(Some(&public)));
        assert_eq!(
            status,
            Status::Licensed {
                email: "buyer@example.com".into(),
                plan: "standard".into()
            }
        );
        // Without a public key in the build, a stored key cannot count.
        assert_eq!(
            evaluate(&record, start + 400 * DAY, offline(None)).0,
            Status::Expired
        );
        // A stored key that is not ours leaves the trial rules in force.
        let bad = Record {
            key: Some("junk".into()),
            ..record
        };
        assert_eq!(
            evaluate(&bad, start + DAY, offline(Some(&public))).0,
            Status::Trial { days_left: 13 }
        );
    }

    const DEVICE: &str = "this-mac";

    fn receipt(signing: &SigningKey, key: &str, device: &str) -> String {
        let payload = format!(r#"{{"licence":"{}","device":"{device}"}}"#, key_id(key));
        issue(signing, &payload)
    }

    #[test]
    fn a_key_has_one_id_however_it_was_pasted() {
        let (signing, _) = test_pair();
        let key = issue(&signing, PAYLOAD);
        let wrapped = format!(" {}\n{} ", &key[..40], &key[40..]);
        assert_eq!(key_id(&wrapped), key_id(&key));
        assert_eq!(key_id(&key).len(), 64);
        // The value the licence service computes for the same text.
        assert_eq!(
            key_id("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn with_a_licence_server_a_key_needs_a_receipt_for_this_mac() {
        let (signing, public) = test_pair();
        let key = issue(&signing, PAYLOAD);
        let start = 1_800_000_000;
        let check = Check {
            public_key: Some(&public),
            activation: Activation::Required(DEVICE),
        };
        let with = |receipt: Option<String>| {
            let record = Record {
                trial_started: Some(start),
                last_seen: Some(start),
                key: Some(key.clone()),
                receipt,
                revoked: Vec::new(),
            };
            evaluate(&record, start + 400 * DAY, check).0
        };
        let licensed = |status: Status| matches!(status, Status::Licensed { .. });

        assert!(licensed(with(Some(receipt(&signing, &key, DEVICE)))));
        // No receipt, one for another Mac, one for another key, a forged one.
        assert_eq!(with(None), Status::Expired);
        assert_eq!(
            with(Some(receipt(&signing, &key, "another-mac"))),
            Status::Expired
        );
        let other_key = issue(&signing, &PAYLOAD.replace("buyer", "other"));
        assert_eq!(
            with(Some(receipt(&signing, &other_key, DEVICE))),
            Status::Expired
        );
        let forger = SigningKey::from_bytes(&[9u8; 32]);
        assert_eq!(with(Some(receipt(&forger, &key, DEVICE))), Status::Expired);
        // A licence key is not a receipt, and a receipt is not a licence key.
        assert_eq!(with(Some(key.clone())), Status::Expired);
        assert_eq!(
            verify(&receipt(&signing, &key, DEVICE), &public),
            Err(KeyError::Malformed)
        );
        // A Mac whose id could not be read matches no receipt.
        let unknown = Check {
            activation: Activation::Required(""),
            ..check
        };
        let record = Record {
            key: Some(key.clone()),
            receipt: Some(receipt(&signing, &key, DEVICE)),
            ..Record::default()
        };
        assert!(!licensed(evaluate(&record, start, unknown).0));
    }

    #[test]
    fn a_refunded_key_stops_counting() {
        let (signing, public) = test_pair();
        let key = issue(&signing, PAYLOAD);
        let start = 1_800_000_000;
        let record = Record {
            trial_started: Some(start),
            last_seen: Some(start),
            key: Some(key.clone()),
            receipt: None,
            revoked: vec!["someone-else".into(), key_id(&key)],
        };
        let (status, next) = evaluate(&record, start + 20 * DAY, offline(Some(&public)));
        assert_eq!(status, Status::Expired);
        // The list is kept for next time.
        assert_eq!(next.revoked, record.revoked);

        let cleared = Record {
            revoked: vec!["someone-else".into()],
            ..record
        };
        assert!(matches!(
            evaluate(&cleared, start + 20 * DAY, offline(Some(&public))).0,
            Status::Licensed { .. }
        ));
    }
}
