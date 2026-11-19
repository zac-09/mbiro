//! The trial and the licence key, both checked on this Mac with no network.
//!
//! A licence key is a small JSON payload and an Ed25519 signature of it, each
//! in URL-safe base64, joined by a dot. The app holds only the public key: it
//! can check a key but never make one.
//!
//! The trial is fourteen days from first launch, with everything on. After
//! that, without a licence: one download at a time, two connections, and no
//! video downloads.
//!
//! Not here yet (M4): the activation call that limits a key to three Macs,
//! and the weekly list of refunded keys.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

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

/// Checks a key against a public key and returns what it says.
pub fn verify(key: &str, public_key_b64: &str) -> Result<Licence, KeyError> {
    // Keys get pasted from emails: spaces and line breaks are forgiven.
    let key: String = key.chars().filter(|c| !c.is_whitespace()).collect();
    let (payload, signature) = key.split_once('.').ok_or(KeyError::Malformed)?;
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

    // Only now is the payload trusted enough to read.
    let licence: Licence = serde_json::from_slice(&payload).map_err(|_| KeyError::Malformed)?;
    let sane = !licence.email.is_empty() && licence.email.len() <= 320 && licence.plan.len() <= 64;
    sane.then_some(licence).ok_or(KeyError::Malformed)
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
}

/// Works out the status at `now`, and what to remember afterwards.
pub fn evaluate(record: &Record, now: i64, public_key: Option<&str>) -> (Status, Record) {
    let now = now.max(record.last_seen.unwrap_or(0));
    let started = record.trial_started.unwrap_or(now);
    let next = Record {
        trial_started: Some(started),
        last_seen: Some(now),
        key: record.key.clone(),
    };

    let licence = match (&record.key, public_key) {
        (Some(key), Some(public)) => verify(key, public).ok(),
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
        let (status, record) = evaluate(&Record::default(), start, None);
        assert_eq!(status, Status::Trial { days_left: 14 });
        assert_eq!(record.trial_started, Some(start));

        let at = |seconds: i64| evaluate(&record, start + seconds, None).0;
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
        let (_, record) = evaluate(&Record::default(), start, None);
        let (status, record) = evaluate(&record, start + 20 * DAY, None);
        assert_eq!(status, Status::Expired);
        // The clock goes back to the first day; the app remembers better.
        assert_eq!(evaluate(&record, start + 60, None).0, Status::Expired);
    }

    #[test]
    fn a_valid_key_lifts_the_limits_at_any_time() {
        let (signing, public) = test_pair();
        let start = 1_800_000_000;
        let record = Record {
            trial_started: Some(start),
            last_seen: Some(start),
            key: Some(issue(&signing, PAYLOAD)),
        };
        let (status, _) = evaluate(&record, start + 400 * DAY, Some(&public));
        assert_eq!(
            status,
            Status::Licensed {
                email: "buyer@example.com".into(),
                plan: "standard".into()
            }
        );
        // Without a public key in the build, a stored key cannot count.
        assert_eq!(
            evaluate(&record, start + 400 * DAY, None).0,
            Status::Expired
        );
        // A stored key that is not ours leaves the trial rules in force.
        let bad = Record {
            key: Some("junk".into()),
            ..record
        };
        assert_eq!(
            evaluate(&bad, start + DAY, Some(&public)).0,
            Status::Trial { days_left: 13 }
        );
    }
}
