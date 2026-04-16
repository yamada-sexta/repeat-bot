use regex::Regex;
use std::sync::LazyLock;
use url::Url;

/// Regex to extract URLs from message text.
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://[^\s<>\)\]\}]+").unwrap()
});

/// Query parameters that are almost always tracking/session junk and never
/// part of the actual content identity.
const JUNK_PARAMS: &[&str] = &[
    // Social / share trackers
    "s",
    "t",
    "si",
    "ref",
    "ref_src",
    "ref_url",
    "src",
    "source",
    "feature",
    // UTM family
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "utm_cid",
    // Facebook / Instagram
    "fbclid",
    "igsh",
    "igshid",
    "mibextid",
    // Google
    "gclid",
    "gbraid",
    "wbraid",
    "dclid",
    // TikTok
    "is_from_webapp",
    "sender_device",
    "is_copy_url",
    "_t",
    // Reddit
    "share_id",
    "context",
    // YouTube
    "pp",
    "sttick",
    // Misc
    "mc_cid",
    "mc_eid",
    "oly_anon_id",
    "oly_enc_id",
    "vero_id",
    "nr_email_referer",
    "_hsenc",
    "_hsmi",
    "hsa_acc",
    "hsa_cam",
    "hsa_grp",
    "hsa_ad",
    "hsa_src",
    "hsa_tgt",
    "hsa_kw",
    "hsa_mt",
    "hsa_net",
    "hsa_ver",
];

/// Extract all URLs from a message and return their normalized forms.
pub fn extract_and_normalize_urls(text: &str) -> Vec<String> {
    URL_RE
        .find_iter(text)
        .filter_map(|m| normalize_url(m.as_str()))
        .collect()
}

/// Normalize a URL to a canonical form so that cosmetically-different links
/// pointing to the same content compare as equal.
fn normalize_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;

    // -- Host normalization --------------------------------------------------
    let host = url.host_str()?.to_lowercase();

    // Unify Twitter domains
    let host = unify_host(&host);

    url.set_host(Some(&host)).ok()?;

    // Strip "www." prefix
    if let Some(stripped) = url.host_str().and_then(|h| h.strip_prefix("www.")) {
        let stripped = stripped.to_string();
        url.set_host(Some(&stripped)).ok()?;
    }

    // -- Path normalization --------------------------------------------------
    // Strip trailing slashes from the path
    let path = url.path().trim_end_matches('/').to_string();
    url.set_path(if path.is_empty() { "/" } else { &path });

    // Platform-specific path rewrites
    rewrite_path_for_platform(&mut url);

    // -- Query parameter cleanup ---------------------------------------------
    strip_junk_query_params(&mut url);

    // -- Fragment removal ----------------------------------------------------
    url.set_fragment(None);

    // -- Scheme normalization ------------------------------------------------
    // Force https
    let _ = url.set_scheme("https");

    Some(url.to_string())
}

/// Map alternative hostnames to a single canonical host.
fn unify_host(host: &str) -> String {
    let host = host.strip_prefix("www.").unwrap_or(host);

    match host {
        // Twitter / X
        "twitter.com" | "mobile.twitter.com" | "mobile.x.com" | "fxtwitter.com"
        | "vxtwitter.com" | "fixvx.com" | "nitter.net" => "x.com".to_string(),

        // Reddit
        "old.reddit.com" | "new.reddit.com" | "np.reddit.com" | "i.reddit.com"
        | "m.reddit.com" | "amp.reddit.com" | "vxreddit.com" | "rxddit.com" => {
            "reddit.com".to_string()
        }

        // YouTube
        "m.youtube.com" | "youtu.be" => "youtube.com".to_string(),

        // Instagram
        "ddinstagram.com" | "m.instagram.com" => "instagram.com".to_string(),

        // TikTok
        "m.tiktok.com" | "vm.tiktok.com" | "vt.tiktok.com" => "tiktok.com".to_string(),

        other => other.to_string(),
    }
}

/// Platform-specific path rewrites (e.g., youtu.be short links → full path).
fn rewrite_path_for_platform(url: &mut Url) {
    let host = url.host_str().unwrap_or_default().to_string();

    match host.as_str() {
        "youtube.com" => {
            // youtu.be/<ID> → youtube.com/watch?v=<ID>
            // The host was already rewritten, but the path is still /<ID>
            let path = url.path().to_string();
            if !path.starts_with("/watch")
                && !path.starts_with("/shorts")
                && !path.starts_with("/playlist")
                && !path.starts_with("/channel")
                && !path.starts_with("/@")
                && !path.starts_with("/embed")
                && path.len() > 1
            {
                // Looks like a youtu.be/<VIDEO_ID> path
                let video_id = path.trim_start_matches('/');
                if !video_id.is_empty() && !video_id.contains('/') {
                    url.set_path("/watch");
                    // Preserve existing query pairs and prepend v=<id>
                    let existing: Vec<(String, String)> = url
                        .query_pairs()
                        .map(|(k, v)| (k.into_owned(), v.into_owned()))
                        .collect();
                    url.query_pairs_mut()
                        .clear()
                        .append_pair("v", video_id)
                        .extend_pairs(existing);
                }
            }

            // For /watch URLs keep only "v" and "list" params
            if url.path() == "/watch" {
                keep_only_params(url, &["v", "list"]);
            }
            // For /shorts keep nothing
            if url.path().starts_with("/shorts") {
                url.set_query(None);
            }
        }
        "x.com" => {
            // Twitter: /user/status/<id> — strip everything after
            // Also handle /i/status/<id>
            // Keep path as-is (the query params are handled by JUNK_PARAMS)
        }
        _ => {}
    }
}

/// Remove tracking / junk query parameters.
fn strip_junk_query_params(url: &mut Url) {
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| {
            let k = key.to_lowercase();
            !JUNK_PARAMS.contains(&k.as_str())
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if pairs.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(&pairs);
    }
}

/// Keep only the specified query parameter keys, removing everything else.
fn keep_only_params(url: &mut Url, keep: &[&str]) {
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| keep.contains(&key.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if pairs.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(&pairs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twitter_tracking_stripped() {
        let urls = extract_and_normalize_urls(
            "check this out https://x.com/manmilk2/status/2044611743083569224?s=12",
        );
        assert_eq!(urls.len(), 1);
        assert_eq!(
            urls[0],
            "https://x.com/manmilk2/status/2044611743083569224"
        );
    }

    #[test]
    fn twitter_domain_unification() {
        let a = extract_and_normalize_urls(
            "https://twitter.com/user/status/123",
        );
        let b = extract_and_normalize_urls(
            "https://x.com/user/status/123",
        );
        let c = extract_and_normalize_urls(
            "https://vxtwitter.com/user/status/123",
        );
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn youtube_short_link() {
        let a = extract_and_normalize_urls("https://youtu.be/dQw4w9WgXcQ");
        let b = extract_and_normalize_urls("https://www.youtube.com/watch?v=dQw4w9WgXcQ&pp=abc");
        assert_eq!(a, b);
        assert_eq!(a[0], "https://youtube.com/watch?v=dQw4w9WgXcQ");
    }

    #[test]
    fn youtube_with_list_preserved() {
        let urls = extract_and_normalize_urls(
            "https://youtube.com/watch?v=abc123&list=PLxyz&utm_source=share",
        );
        assert_eq!(urls[0], "https://youtube.com/watch?v=abc123&list=PLxyz");
    }

    #[test]
    fn reddit_domain_unification() {
        let a = extract_and_normalize_urls(
            "https://old.reddit.com/r/rust/comments/abc123/cool_post",
        );
        let b = extract_and_normalize_urls(
            "https://www.reddit.com/r/rust/comments/abc123/cool_post/",
        );
        assert_eq!(a, b);
    }

    #[test]
    fn utm_params_stripped() {
        let urls = extract_and_normalize_urls(
            "https://example.com/article?id=42&utm_source=twitter&utm_medium=social",
        );
        assert_eq!(urls[0], "https://example.com/article?id=42");
    }

    #[test]
    fn fbclid_stripped() {
        let urls = extract_and_normalize_urls(
            "https://example.com/page?fbclid=abc123def456",
        );
        assert_eq!(urls[0], "https://example.com/page");
    }

    #[test]
    fn www_stripped() {
        let a = extract_and_normalize_urls("https://www.example.com/page");
        let b = extract_and_normalize_urls("https://example.com/page");
        assert_eq!(a, b);
    }

    #[test]
    fn http_upgraded_to_https() {
        let urls = extract_and_normalize_urls("http://example.com/page");
        assert_eq!(urls[0], "https://example.com/page");
    }

    #[test]
    fn trailing_slash_stripped() {
        let a = extract_and_normalize_urls("https://example.com/page/");
        let b = extract_and_normalize_urls("https://example.com/page");
        assert_eq!(a, b);
    }

    #[test]
    fn fragment_stripped() {
        let urls = extract_and_normalize_urls("https://example.com/page#section-2");
        assert_eq!(urls[0], "https://example.com/page");
    }

    #[test]
    fn multiple_urls_in_message() {
        let urls = extract_and_normalize_urls(
            "look at https://x.com/user/status/111?s=12 and also https://youtu.be/abc",
        );
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn instagram_domain_unification() {
        let a = extract_and_normalize_urls("https://ddinstagram.com/p/abc123");
        let b = extract_and_normalize_urls("https://instagram.com/p/abc123");
        assert_eq!(a, b);
    }

    #[test]
    fn tiktok_domain_unification() {
        let a = extract_and_normalize_urls("https://vm.tiktok.com/abc123");
        let b = extract_and_normalize_urls("https://tiktok.com/abc123");
        assert_eq!(a, b);
    }
}
