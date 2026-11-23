//! The other people's work this app is made with, and the terms it comes under.
//!
//! Two kinds: what ships inside the app (aria2 and the two typefaces), whose
//! licence texts are compiled in from `licences/` and also copied into the
//! bundle, and the video tools the app fetches on request, which are named
//! with a link to their source and licence.
//!
//! aria2 is GPL software shipped as a program of its own. Its entry carries
//! the licence, where the exact source came from, and the script that built
//! it, so anyone can make the same program.

use serde::Serialize;

use crate::{strings, tools};

/// Keep in step with `ARIA2_VERSION`, `ARIA2_SHA256` and `ARIA2_URL` in
/// scripts/build-aria2.sh; a test below fails if they drift.
pub const ARIA2_VERSION: &str = "1.37.0";
pub const ARIA2_SHA256: &str = "60a420ad7085eb616cb6e2bdf0a7206d68ff3d37fb5a956dc44242eb2f79b66b";
pub const ARIA2_SOURCE: &str =
    "https://github.com/aria2/aria2/releases/download/release-1.37.0/aria2-1.37.0.tar.xz";

const ARIA2_BUILD_SCRIPT: &str = include_str!("../../../../scripts/build-aria2.sh");
const ARIA2_COPYING: &str = include_str!("../licences/aria2-COPYING.txt");
const ARIA2_OPENSSL_EXCEPTION: &str = include_str!("../licences/aria2-LICENSE.OpenSSL.txt");
const BARLOW_LICENCE: &str = include_str!("../licences/barlow-condensed-OFL.txt");
const PLEX_LICENCE: &str = include_str!("../licences/ibm-plex-mono-OFL.txt");

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Acknowledgement {
    pub name: &'static str,
    /// Absent for what keeps itself up to date.
    pub version: Option<&'static str>,
    /// The licence's short name.
    pub licence: &'static str,
    /// What it does here, in a sentence.
    pub role: &'static str,
    /// True if it is inside the app; false if the app fetches it on request.
    pub shipped: bool,
    /// Where its source code is.
    pub source: &'static str,
    /// Where its licence can be read, when the text is not carried here.
    pub licence_link: Option<&'static str>,
    /// The licence in full, and for aria2 how it was built.
    pub text: Option<String>,
}

fn aria2_text() -> String {
    format!(
        "{}\n\n  {ARIA2_SOURCE}\n  SHA-256 {ARIA2_SHA256}\n\n{}\n\n\
         ================ COPYING ================\n\n{ARIA2_COPYING}\n\
         ================ LICENSE.OpenSSL ================\n\n{ARIA2_OPENSSL_EXCEPTION}\n\
         ================ scripts/build-aria2.sh ================\n\n{ARIA2_BUILD_SCRIPT}",
        strings::ACK_ARIA2_SOURCE,
        strings::ACK_ARIA2_BUILD,
    )
}

pub fn all() -> Vec<Acknowledgement> {
    vec![
        Acknowledgement {
            name: "aria2",
            version: Some(ARIA2_VERSION),
            licence: "GPL-2.0-or-later",
            role: strings::ACK_ARIA2_ROLE,
            shipped: true,
            source: ARIA2_SOURCE,
            licence_link: None,
            text: Some(aria2_text()),
        },
        Acknowledgement {
            name: "Barlow Condensed",
            version: None,
            licence: "SIL Open Font License 1.1",
            role: strings::ACK_FONT_ROLE,
            shipped: true,
            source: "https://github.com/jpt/barlow",
            licence_link: None,
            text: Some(BARLOW_LICENCE.to_owned()),
        },
        Acknowledgement {
            name: "IBM Plex Mono",
            version: None,
            licence: "SIL Open Font License 1.1",
            role: strings::ACK_FONT_ROLE,
            shipped: true,
            source: "https://github.com/IBM/plex",
            licence_link: None,
            text: Some(PLEX_LICENCE.to_owned()),
        },
        Acknowledgement {
            name: tools::YT_DLP.name,
            version: None,
            licence: "Unlicense",
            role: strings::ACK_YT_DLP_ROLE,
            shipped: false,
            source: "https://github.com/yt-dlp/yt-dlp",
            licence_link: Some("https://github.com/yt-dlp/yt-dlp/blob/master/LICENSE"),
            text: None,
        },
        Acknowledgement {
            name: "Deno",
            version: Some(tools::DENO.version),
            licence: "MIT",
            role: strings::ACK_DENO_ROLE,
            shipped: false,
            source: "https://github.com/denoland/deno",
            licence_link: Some("https://github.com/denoland/deno/blob/main/LICENSE.md"),
            text: None,
        },
        Acknowledgement {
            name: "FFmpeg",
            version: Some(tools::FFMPEG.version),
            licence: "GPL-3.0-or-later",
            role: strings::ACK_FFMPEG_ROLE,
            shipped: false,
            // The build the app fetches, and the scripts that made it.
            source: "https://git.martin-riedl.de/ffmpeg/build-script",
            licence_link: Some("https://www.gnu.org/licenses/gpl-3.0.html"),
            text: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aria2_s_entry_names_exactly_what_the_build_script_builds() {
        for line in [
            format!("ARIA2_VERSION=\"{ARIA2_VERSION}\""),
            format!("ARIA2_SHA256=\"{ARIA2_SHA256}\""),
        ] {
            assert!(ARIA2_BUILD_SCRIPT.contains(&line), "missing {line}");
        }
        // The script builds its URL from the version; ours must be that URL.
        assert_eq!(
            ARIA2_SOURCE,
            format!(
                "https://github.com/aria2/aria2/releases/download/release-{ARIA2_VERSION}/aria2-{ARIA2_VERSION}.tar.xz"
            )
        );
    }

    #[test]
    fn everything_shipped_carries_its_licence_in_full() {
        let all = all();
        for entry in &all {
            assert!(entry.source.starts_with("https://"), "{}", entry.name);
            if entry.shipped {
                let text = entry.text.as_deref().expect("a shipped licence text");
                assert!(text.len() > 4000, "{} has no full licence", entry.name);
            } else {
                assert!(
                    entry.licence_link.is_some(),
                    "{} has no licence link",
                    entry.name
                );
            }
        }
        let aria2 = all[0].text.as_deref().unwrap_or_default();
        assert!(aria2.contains("GNU GENERAL PUBLIC LICENSE"));
        assert!(aria2.contains("Version 2, June 1991"));
        assert!(aria2.contains(ARIA2_SHA256));
        assert!(aria2.contains("--disable-bittorrent"));
        for font in &all[1..3] {
            assert!(font
                .text
                .as_deref()
                .unwrap_or_default()
                .contains("SIL OPEN FONT LICENSE"));
        }
    }

    #[test]
    fn every_tool_the_app_fetches_is_acknowledged() {
        let named: Vec<String> = all().iter().map(|a| a.name.to_lowercase()).collect();
        for tool in tools::ALL {
            assert!(
                named.contains(&tool.name.to_lowercase()),
                "{} is missing",
                tool.name
            );
        }
        assert!(named.contains(&"aria2".to_owned()));
    }
}
