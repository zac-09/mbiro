//! What the add dialog needs to know about a video page before downloading it:
//! its title, length, picture, and the qualities on offer.
//!
//! All of it comes from asking yt-dlp to describe the page. Nothing here knows
//! about any particular site.

use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use url::Url;

use crate::engine::ytdlp::ToolPaths;
use crate::engine::VideoOptions;

/// The first run of yt-dlp after it is installed can take ten seconds on its own.
const PROBE_TIMEOUT: Duration = Duration::from_secs(90);
const THUMBNAIL_TIMEOUT: Duration = Duration::from_secs(10);
const THUMBNAIL_MAX_BYTES: usize = 3 * 1024 * 1024;

/// The heights offered below "best", when the page has something taller.
const STEPS: [u32; 2] = [720, 480];

/// What the user picked. Built into a yt-dlp format selector here, never in the window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Quality {
    Best,
    /// The best picture no taller than this.
    Max {
        height: u32,
    },
    Audio,
    /// One exact format from the full list.
    Format {
        id: String,
        audio_only: bool,
    },
}

impl Quality {
    /// The yt-dlp options for this choice, or `None` if it is not a choice we offer.
    pub fn options(&self) -> Option<VideoOptions> {
        let video = |format: String| VideoOptions {
            format,
            audio_only: false,
        };
        match self {
            Self::Best => Some(video("bv*+ba/b".into())),
            Self::Max { height } if (144..=4320).contains(height) => Some(video(format!(
                "bv*[height<={height}]+ba/b[height<={height}]"
            ))),
            Self::Max { .. } => None,
            Self::Audio => Some(VideoOptions {
                format: "ba[ext=m4a]/ba".into(),
                audio_only: true,
            }),
            Self::Format { id, audio_only } => {
                // A format id is a short token; anything else is not passed on.
                let plain = !id.is_empty()
                    && id.len() <= 64
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
                if !plain {
                    None
                } else if *audio_only {
                    Some(VideoOptions {
                        format: id.clone(),
                        audio_only: true,
                    })
                } else {
                    // A picture-only format gets the best sound added.
                    Some(video(format!("{id}+ba/{id}")))
                }
            }
        }
    }
}

/// One line of the short quality picker.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub quality: Quality,
    pub height: Option<u32>,
    pub ext: String,
    pub approx_bytes: Option<u64>,
}

/// One line of "show all formats".
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatRow {
    pub quality: Quality,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub ext: String,
    pub has_video: bool,
    pub has_audio: bool,
    pub approx_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub title: String,
    pub duration_secs: Option<u64>,
    pub host: String,
    /// The page's picture as a `data:` URL, so the window loads nothing itself.
    pub thumbnail: Option<String>,
    pub presets: Vec<Preset>,
    pub formats: Vec<FormatRow>,
}

/// One video of a playlist.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub url: String,
    pub title: String,
    pub duration_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistInfo {
    pub title: String,
    pub host: String,
    pub entries: Vec<PlaylistEntry>,
}

/// What a link turned out to be.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Found {
    Video(VideoInfo),
    Playlist(PlaylistInfo),
}

/// More entries than this are not listed; nobody ticks through thousands.
const PLAYLIST_MAX_ENTRIES: usize = 2000;

/// Turns yt-dlp's flat description of a playlist into the checklist the dialog shows.
pub fn describe_playlist(json: &Value, page: &Url) -> PlaylistInfo {
    let entries = json["entries"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|entry| {
                    let address = entry["url"]
                        .as_str()
                        .or_else(|| entry["webpage_url"].as_str())?;
                    // Each entry becomes its own download, so it must be an ordinary link.
                    let url = Url::parse(address)
                        .ok()
                        .filter(|u| matches!(u.scheme(), "http" | "https"))?;
                    let title = entry["title"].as_str().filter(|t| !t.is_empty());
                    Some(PlaylistEntry {
                        title: title.unwrap_or(address).to_owned(),
                        url: url.into(),
                        duration_secs: entry["duration"].as_f64().map(|d| d.round() as u64),
                    })
                })
                .take(PLAYLIST_MAX_ENTRIES)
                .collect()
        })
        .unwrap_or_default();
    PlaylistInfo {
        title: json["title"].as_str().unwrap_or_default().to_owned(),
        host: page
            .host_str()
            .unwrap_or_default()
            .trim_start_matches("www.")
            .to_owned(),
        entries,
    }
}

/// A format as yt-dlp describes it, reduced to what we use.
struct Format {
    id: String,
    ext: String,
    height: Option<u32>,
    fps: Option<u32>,
    has_video: bool,
    has_audio: bool,
    bytes: Option<u64>,
    /// Total bitrate in kbit/s, for ranking.
    rate: f64,
}

fn read_format(value: &Value, duration: Option<f64>) -> Option<Format> {
    let codec = |key: &str| value[key].as_str().is_some_and(|c| c != "none");
    let (has_video, has_audio) = (codec("vcodec"), codec("acodec"));
    if !has_video && !has_audio {
        return None; // storyboards and the like
    }
    let rate = value["tbr"].as_f64().unwrap_or(0.0);
    let bytes = value["filesize"]
        .as_f64()
        .or_else(|| value["filesize_approx"].as_f64())
        .or_else(|| {
            duration
                .filter(|_| rate > 0.0)
                .map(|secs| rate * 125.0 * secs)
        })
        .map(|b| b as u64);
    Some(Format {
        id: value["format_id"].as_str()?.to_owned(),
        ext: value["ext"].as_str().unwrap_or_default().to_owned(),
        height: value["height"]
            .as_u64()
            .map(|h| h as u32)
            .filter(|_| has_video),
        fps: value["fps"]
            .as_f64()
            .map(|f| f.round() as u32)
            .filter(|_| has_video),
        has_video,
        has_audio,
        bytes,
        rate,
    })
}

/// Turns yt-dlp's description of a page into what the dialog shows.
/// The thumbnail is fetched separately.
pub fn describe(json: &Value, page: &Url) -> VideoInfo {
    let duration = json["duration"].as_f64();
    let formats: Vec<Format> = json["formats"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|f| read_format(f, duration))
                .collect()
        })
        .unwrap_or_default();

    let best_audio = formats
        .iter()
        .filter(|f| f.has_audio && !f.has_video)
        .max_by(|a, b| a.rate.total_cmp(&b.rate));
    let audio_bytes = best_audio.and_then(|f| f.bytes);
    // The picture yt-dlp would take for a height limit, and the size with sound added.
    let pick = |limit: Option<u32>| {
        formats
            .iter()
            .filter(|f| f.has_video && limit.is_none_or(|max| f.height.is_some_and(|h| h <= max)))
            .max_by(|a, b| (a.height, a.rate as u64).cmp(&(b.height, b.rate as u64)))
            .map(|f| {
                let bytes = match (f.bytes, f.has_audio, audio_bytes) {
                    (Some(video), false, Some(audio)) => Some(video + audio),
                    (bytes, _, _) => bytes,
                };
                (f.height, bytes)
            })
    };

    let mut presets = Vec::new();
    let best = pick(None);
    if let Some((height, approx_bytes)) = best {
        presets.push(Preset {
            quality: Quality::Best,
            height,
            ext: "mp4".into(),
            approx_bytes,
        });
        for step in STEPS {
            // A lower step is only worth showing if it is really lower.
            if height.is_some_and(|h| h > step) {
                if let Some((height, approx_bytes)) = pick(Some(step)) {
                    let quality = Quality::Max { height: step };
                    presets.push(Preset {
                        quality,
                        height,
                        ext: "mp4".into(),
                        approx_bytes,
                    });
                }
            }
        }
    }
    if let Some(audio) = best_audio.or_else(|| formats.iter().find(|f| f.has_audio)) {
        presets.push(Preset {
            quality: Quality::Audio,
            height: None,
            ext: if audio.has_video {
                "m4a".into()
            } else {
                audio.ext.clone()
            },
            approx_bytes: if audio.has_video { None } else { audio.bytes },
        });
    }
    if presets.is_empty() {
        // yt-dlp listed no formats it could describe; it can still try its best.
        let ext = json["ext"].as_str().unwrap_or("mp4").to_owned();
        presets.push(Preset {
            quality: Quality::Best,
            height: None,
            ext,
            approx_bytes: None,
        });
    }

    // Tallest first, then sound-only.
    let mut rows: Vec<&Format> = formats.iter().collect();
    rows.sort_by(|a, b| {
        (b.has_video, b.height, b.rate as u64).cmp(&(a.has_video, a.height, a.rate as u64))
    });
    let formats = rows
        .into_iter()
        .map(|f| FormatRow {
            quality: Quality::Format {
                id: f.id.clone(),
                audio_only: !f.has_video,
            },
            height: f.height,
            fps: f.fps,
            ext: f.ext.clone(),
            has_video: f.has_video,
            has_audio: f.has_audio,
            approx_bytes: f.bytes,
        })
        .collect();

    VideoInfo {
        title: json["title"].as_str().unwrap_or_default().to_owned(),
        duration_secs: duration.map(|d| d.round() as u64),
        host: page
            .host_str()
            .unwrap_or_default()
            .trim_start_matches("www.")
            .to_owned(),
        thumbnail: None,
        presets,
        formats,
    }
}

/// Asks yt-dlp about a page. The error is yt-dlp's own last line, for the user.
pub async fn probe(tools: &ToolPaths, page: &Url) -> Result<Found, String> {
    let run = Command::new(&tools.yt_dlp)
        // A link to one video that also names a playlist is treated as the
        // video. A playlist is only listed, not looked into, which keeps this quick.
        .args(["--ignore-config", "--no-playlist", "--flat-playlist"])
        .args(["--no-warnings", "--dump-single-json"])
        .arg("--js-runtimes")
        .arg(format!("deno:{}", tools.deno.display()))
        .arg("--ffmpeg-location")
        .arg(&tools.ffmpeg)
        .arg("--")
        .arg(page.as_str())
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(PROBE_TIMEOUT, run)
        .await
        .map_err(|_| crate::strings::VIDEO_PROBE_SLOW.to_owned())?
        .map_err(|_| crate::strings::VIDEO_PROBE_FAILED.to_owned())?;
    if !output.status.success() {
        let log = String::from_utf8_lossy(&output.stderr);
        let reason = log
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix("ERROR: "));
        return Err(reason
            .unwrap_or(crate::strings::VIDEO_PROBE_FAILED)
            .to_owned());
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| crate::strings::VIDEO_PROBE_FAILED.to_owned())?;

    if json["_type"].as_str() == Some("playlist") {
        let playlist = describe_playlist(&json, page);
        if playlist.entries.is_empty() {
            return Err(crate::strings::VIDEO_PROBE_FAILED.to_owned());
        }
        return Ok(Found::Playlist(playlist));
    }
    let mut info = describe(&json, page);
    if let Some(address) = json["thumbnail"].as_str() {
        info.thumbnail = fetch_thumbnail(address).await;
    }
    Ok(Found::Video(info))
}

/// Fetches the page's picture, part of the look-up the user asked for. Any
/// problem just means no picture.
async fn fetch_thumbnail(address: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .https_only(true)
        .timeout(THUMBNAIL_TIMEOUT)
        .build()
        .ok()?;
    let mut response = client
        .get(address)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let kind = response
        .headers()
        .get("content-type")?
        .to_str()
        .ok()?
        .to_owned();
    let kind = kind.split(';').next()?.trim().to_owned();
    if !matches!(kind.as_str(), "image/jpeg" | "image/png" | "image/webp") {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > THUMBNAIL_MAX_BYTES {
            return None;
        }
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{kind};base64,{encoded}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page() -> Url {
        "https://www.example.com/talks/queues".parse().unwrap()
    }

    fn sample() -> Value {
        json!({
            "title": "Building reliable job queues",
            "duration": 2538.0,
            "formats": [
                { "format_id": "sb0", "ext": "mhtml", "vcodec": "none", "acodec": "none" },
                { "format_id": "140", "ext": "m4a", "vcodec": "none", "acodec": "mp4a", "tbr": 129.0, "filesize": 40_000_000 },
                { "format_id": "139", "ext": "m4a", "vcodec": "none", "acodec": "mp4a", "tbr": 48.0, "filesize": 15_000_000 },
                { "format_id": "135", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "height": 480, "fps": 30.0, "tbr": 600.0, "filesize": 190_000_000 },
                { "format_id": "136", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "height": 720, "fps": 30.0, "tbr": 1300.0, "filesize_approx": 410_000_000 },
                { "format_id": "137", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "height": 1080, "fps": 29.97, "tbr": 2200.0 },
                { "format_id": "18", "ext": "mp4", "vcodec": "avc1", "acodec": "mp4a", "height": 360, "fps": 30.0, "tbr": 500.0, "filesize": 150_000_000 },
            ],
        })
    }

    #[test]
    fn offers_best_then_lower_steps_then_audio() {
        let info = describe(&sample(), &page());
        assert_eq!(info.title, "Building reliable job queues");
        assert_eq!(info.duration_secs, Some(2538));
        assert_eq!(info.host, "example.com");

        let kinds: Vec<_> = info.presets.iter().map(|p| p.quality.clone()).collect();
        assert_eq!(
            kinds,
            vec![
                Quality::Best,
                Quality::Max { height: 720 },
                Quality::Max { height: 480 },
                Quality::Audio
            ]
        );
        // Best is 1080p; its size is worked out from the bitrate, plus the sound.
        assert_eq!(info.presets[0].height, Some(1080));
        assert_eq!(
            info.presets[0].approx_bytes,
            Some((2200.0 * 125.0 * 2538.0) as u64 + 40_000_000)
        );
        assert_eq!(info.presets[1].approx_bytes, Some(450_000_000));
        assert_eq!(info.presets[3].ext, "m4a");
        assert_eq!(info.presets[3].approx_bytes, Some(40_000_000));
    }

    #[test]
    fn lists_every_real_format_tallest_first() {
        let info = describe(&sample(), &page());
        let ids: Vec<_> = info
            .formats
            .iter()
            .map(|row| match &row.quality {
                Quality::Format { id, .. } => id.as_str(),
                _ => "",
            })
            .collect();
        assert_eq!(ids, ["137", "136", "135", "18", "140", "139"]);
        assert_eq!(info.formats[0].fps, Some(30));
        assert!(info.formats[3].has_audio && info.formats[3].has_video);
    }

    #[test]
    fn a_short_clip_offers_no_step_it_cannot_beat() {
        let json = json!({ "title": "t", "formats": [
            { "format_id": "v", "ext": "mp4", "vcodec": "h264", "acodec": "aac", "height": 480 },
        ]});
        let info = describe(&json, &page());
        let kinds: Vec<_> = info.presets.iter().map(|p| p.quality.clone()).collect();
        assert_eq!(kinds, vec![Quality::Best, Quality::Audio]);

        let bare = describe(&json!({ "title": "t" }), &page());
        assert_eq!(
            bare.presets.len(),
            1,
            "with no format list, best is still offered"
        );
    }

    #[test]
    fn lists_a_playlist_and_skips_what_is_not_a_link() {
        let json = json!({
            "_type": "playlist",
            "title": "Queue talks",
            "entries": [
                { "url": "https://example.com/watch/1", "title": "Part one", "duration": 61.4 },
                { "webpage_url": "https://example.com/watch/2", "title": "" },
                { "url": "ytsearch:not a link", "title": "Odd" },
                { "title": "No address" },
            ],
        });
        let playlist = describe_playlist(&json, &page());
        assert_eq!(playlist.title, "Queue talks");
        assert_eq!(playlist.host, "example.com");
        assert_eq!(
            playlist.entries,
            vec![
                PlaylistEntry {
                    url: "https://example.com/watch/1".into(),
                    title: "Part one".into(),
                    duration_secs: Some(61),
                },
                PlaylistEntry {
                    url: "https://example.com/watch/2".into(),
                    title: "https://example.com/watch/2".into(),
                    duration_secs: None,
                },
            ]
        );
    }

    #[test]
    fn builds_selectors_and_refuses_anything_odd() {
        let format = |q: Quality| q.options().map(|o| (o.format, o.audio_only));
        assert_eq!(format(Quality::Best), Some(("bv*+ba/b".into(), false)));
        assert_eq!(
            format(Quality::Max { height: 720 }),
            Some(("bv*[height<=720]+ba/b[height<=720]".into(), false))
        );
        assert_eq!(
            format(Quality::Audio),
            Some(("ba[ext=m4a]/ba".into(), true))
        );
        let pick = |id: &str, audio_only| Quality::Format {
            id: id.into(),
            audio_only,
        };
        assert_eq!(
            format(pick("137", false)),
            Some(("137+ba/137".into(), false))
        );
        assert_eq!(format(pick("140", true)), Some(("140".into(), true)));
        assert_eq!(
            format(pick("hls-1080p_v1.2", false)).map(|f| f.1),
            Some(false)
        );

        assert_eq!(format(pick("137 --exec rm", false)), None);
        assert_eq!(format(pick("a/b", false)), None);
        assert_eq!(format(pick("", false)), None);
        assert_eq!(format(Quality::Max { height: 0 }), None);
    }
}
