//! Turning the `href` of a link into something the crawler can fetch.
//!
//! Every spider used to do this by hand with string prefixes, each slightly
//! differently, and none of them handled `../`, query strings or fragments.
//! The `url` crate does all of that correctly, given the URL of the page the
//! link was found on.

use url::Url;

/// Resolve `href` against the page it appeared on.
///
/// Returns `None` for links that are not pages to crawl: empty hrefs,
/// same-page fragments, `mailto:`/`tel:`/`javascript:` and anything that
/// does not end up as `http` or `https`. Fragments are stripped so that
/// `/about` and `/about#team` dedupe to the same URL.
pub fn resolve(page_url: &str, href: &str) -> Option<String> {
    let href = href.trim();

    if href.is_empty()
        || href.starts_with('#')
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
        || href.starts_with("javascript:")
    {
        return None;
    }

    let base = Url::parse(page_url).ok()?;
    let mut url = base.join(href).ok()?;
    url.set_fragment(None);

    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }

    Some(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "https://www.example.com/list/page-1.html?sort=desc";

    #[test]
    fn resolves_absolute_root_relative_and_relative_links() {
        assert_eq!(
            resolve(PAGE, "https://other.example.org/x"),
            Some("https://other.example.org/x".to_string())
        );
        assert_eq!(
            resolve(PAGE, "/cve/CVE-2024-0001/"),
            Some("https://www.example.com/cve/CVE-2024-0001/".to_string())
        );
        assert_eq!(
            resolve(PAGE, "page-2.html"),
            Some("https://www.example.com/list/page-2.html".to_string())
        );
        assert_eq!(
            resolve(PAGE, "../about"),
            Some("https://www.example.com/about".to_string())
        );
    }

    #[test]
    fn resolves_protocol_relative_links() {
        assert_eq!(
            resolve(PAGE, "//www.example.com/cwe-details/79/"),
            Some("https://www.example.com/cwe-details/79/".to_string())
        );
    }

    #[test]
    fn keeps_query_strings_and_drops_fragments() {
        assert_eq!(
            resolve(PAGE, "/list.php?page=2#top"),
            Some("https://www.example.com/list.php?page=2".to_string())
        );
    }

    #[test]
    fn ignores_links_that_are_not_pages() {
        for href in [
            "",
            "   ",
            "#top",
            "mailto:a@b.c",
            "tel:123",
            "javascript:void(0)",
        ] {
            assert_eq!(resolve(PAGE, href), None, "{href:?}");
        }
        assert_eq!(resolve(PAGE, "ftp://files.example.com/x"), None);
    }

    #[test]
    fn unparseable_base_yields_nothing() {
        assert_eq!(resolve("not a url", "/x"), None);
    }
}
