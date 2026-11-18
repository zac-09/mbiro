//! Notices when a link is copied, so the menu bar popover can offer to download it.
//!
//! The pasteboard is only read when its change count moves, never on a timer,
//! and never when the copying app has marked the contents as concealed (as
//! password managers do). Nothing read here is logged or stored; a copied link
//! is held in memory until it is used or replaced. No network call is made:
//! whether a link is worth downloading is the user's call.

use url::Url;

/// The longest link worth offering.
const MAX_LINK: usize = 2048;

/// The pasteboard's change count. It goes up every time anything is copied.
#[cfg(target_os = "macos")]
pub fn change_count() -> isize {
    use objc2_app_kit::NSPasteboard;
    NSPasteboard::generalPasteboard().changeCount()
}

/// The copied text, if there is any and it is not marked as secret.
#[cfg(target_os = "macos")]
pub fn copied_text() -> Option<String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;

    let pasteboard = NSPasteboard::generalPasteboard();
    // A convention password managers follow: see nspasteboard.org.
    let concealed = NSString::from_str("org.nspasteboard.ConcealedType");
    if let Some(types) = pasteboard.types() {
        if types.iter().any(|kind| *kind == *concealed) {
            return None;
        }
    }
    let text = pasteboard.stringForType(unsafe { NSPasteboardTypeString })?;
    Some(text.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn change_count() -> isize {
    0
}

#[cfg(not(target_os = "macos"))]
pub fn copied_text() -> Option<String> {
    None
}

/// The link in a piece of copied text, if the text is exactly one http or https link.
pub fn link_in(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_LINK || text.contains(char::is_whitespace) {
        return None;
    }
    let url = Url::parse(text).ok()?;
    let ordinary = matches!(url.scheme(), "http" | "https") && url.host_str().is_some();
    ordinary.then(|| url.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_only_a_lone_ordinary_link() {
        assert_eq!(
            link_in("  https://example.com/files/site-photos.zip\n").as_deref(),
            Some("https://example.com/files/site-photos.zip")
        );
        assert_eq!(
            link_in("http://example.com").as_deref(),
            Some("http://example.com/")
        );
        assert_eq!(link_in("see https://example.com/a.zip"), None);
        assert_eq!(link_in("hunter2"), None);
        assert_eq!(link_in("magnet:?xt=urn:btih:0"), None);
        assert_eq!(link_in("file:///etc/passwd"), None);
        assert_eq!(link_in(""), None);
        assert_eq!(
            link_in(&format!("https://example.com/{}", "a".repeat(3000))),
            None
        );
    }
}
