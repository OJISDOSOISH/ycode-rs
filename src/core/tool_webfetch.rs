//! Port of the portable part of `opencode/packages/core/src/tool/webfetch.ts`.
//!
//! The HTTP client, the permission check and the timeout are not ported. What
//! is ported is every decision that shapes the request or filters the answer:
//! the Accept header per format, the three request headers, the mime
//! predicates that decide what the tool refuses, the Cloudflare retry rule,
//! and the HTML text extraction.
//!
//! Two details are worth stating plainly, because both are silent behaviour
//! rather than API surface:
//!
//! - `format` is NOT optional on the decoded input. The TS uses
//!   `withDecodingDefault`, so a request that omits it decodes to `"markdown"`
//!   and is re-serialised with it. Here it is a plain field with
//!   `#[serde(default)]`, which reproduces both halves: accepted when absent,
//!   always written when present.
//! - The text extractor counts skipped tags with a single depth counter that
//!   every skipped open AND every skipped close touches, without matching the
//!   tag names. Malformed markup therefore leaks: `<script><style>x</script>y`
//!   leaves `y` skipped, because the `</script>` only brought the depth back
//!   to one. That is the source's logic, so the port keeps it and a test pins
//!   it.
//!
//! Known divergence, recorded rather than hidden: the HTML scanner below is
//! not `htmlparser2`. It handles open tags, close tags, comments and text, and
//! it does NOT handle a `>` inside a quoted attribute value, CDATA sections,
//! implied end tags (`<br>`, `<p>`) or raw-text elements beyond the skip list.
//! Text extraction of well-formed markup matches; pathological markup may not.
//!
//! `convertHTMLToMarkdown` is not ported: it delegates to turndown, and writing
//! a Markdown converter here would be a new implementation rather than a port.
//! Its options and its removal list are recorded below as data so that whoever
//! ports it does not have to guess them.

use serde::{Deserialize, Serialize};

/// Tool name.
pub const NAME: &str = "webfetch";
/// `MAX_RESPONSE_BYTES`.
pub const MAX_RESPONSE_BYTES: u64 = 5 * 1024 * 1024;
/// `DEFAULT_TIMEOUT_SECONDS`.
pub const DEFAULT_TIMEOUT_SECONDS: i64 = 30;
/// `MAX_TIMEOUT_SECONDS`.
pub const MAX_TIMEOUT_SECONDS: i64 = 120;

/// The `User-Agent` sent first, and the one sent on a Cloudflare retry.
pub const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";
/// The retry agent after a challenge.
pub const RETRY_USER_AGENT: &str = "opencode";

/// The two error messages the source produces for a refused content type.
pub const ERR_UNSUPPORTED_IMAGE: &str = "Unsupported fetched image content type";
pub const ERR_UNSUPPORTED_FILE: &str = "Unsupported fetched file content type";
/// The size failure text, without the byte count appended.
pub const ERR_TOO_LARGE: &str = "Response too large (exceeds";

/// `WebFetchTool.description`, as the model reads it.
pub const DESCRIPTION: &str = concat!(
    "Fetch content from an HTTP or HTTPS URL and return it as text, markdown, or HTML. ",
    "Markdown is the default.\n",
    "\n",
    "Use a more targeted tool when one is available. This tool is read-only. ",
    "Large text results may be replaced with a preview while the complete output ",
    "is retained in managed storage.",
);

/// `Schema.Literals(["text", "markdown", "html"])`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Text,
    #[default]
    Markdown,
    Html,
}

impl Format {
    /// The literal as it appears on the wire and in `Output.format`.
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Text => "text",
            Format::Markdown => "markdown",
            Format::Html => "html",
        }
    }
}

/// `WebFetchTool.Input`. `format` defaults to markdown on decode, never absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub url: String,
    #[serde(default)]
    pub format: Format,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i64>,
}

impl Default for Input {
    fn default() -> Self {
        Input {
            url: String::new(),
            format: Format::Markdown,
            timeout: None,
        }
    }
}

impl Input {
    /// The `Timeout` check: strictly positive and at most `MAX_TIMEOUT_SECONDS`.
    pub fn validated(&self) -> Result<Self, &'static str> {
        match self.timeout {
            None => Ok(self.clone()),
            Some(t) if t <= 0 => Err("timeout must be greater than 0"),
            Some(t) if t > MAX_TIMEOUT_SECONDS => Err("timeout exceeds MAX_TIMEOUT_SECONDS"),
            Some(_) => Ok(self.clone()),
        }
    }

    /// `input.timeout ?? DEFAULT_TIMEOUT_SECONDS`.
    pub fn effective_timeout_seconds(&self) -> i64 {
        self.timeout.unwrap_or(DEFAULT_TIMEOUT_SECONDS)
    }
}

/// `WebFetchTool.Output`, with `contentType` kept as one word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    pub url: String,
    #[serde(rename = "contentType")]
    pub content_type: String,
    pub format: Format,
    pub output: String,
}

/// `acceptHeader`, including the unreachable `*/*` tail the switch falls to.
pub fn accept_header(format: Format) -> &'static str {
    match format {
        Format::Markdown => {
            "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1"
        }
        Format::Text => "text/plain;q=1.0, text/markdown;q=0.9, text/html;q=0.8, */*;q=0.1",
        Format::Html => {
            "text/html;q=1.0, application/xhtml+xml;q=0.9, text/plain;q=0.8, text/markdown;q=0.7, */*;q=0.1"
        }
    }
}

/// `headers`, in the order the source builds them.
pub fn headers(format: Format, user_agent: &str) -> Vec<(&'static str, String)> {
    vec![
        ("User-Agent", user_agent.to_string()),
        ("Accept", accept_header(format).to_string()),
        ("Accept-Language", "en-US,en;q=0.9".to_string()),
    ]
}

/// `mimeFrom`: the media type without its parameters, lowercased.
pub fn mime_from(content_type: &str) -> String {
    match content_type.split(';').next() {
        Some(first) => first.trim().to_lowercase(),
        None => String::new(),
    }
}

/// `isImageAttachment`: an image the tool will not hand back as text.
pub fn is_image_attachment(mime: &str) -> bool {
    mime.starts_with("image/") && mime != "image/svg+xml" && mime != "image/vnd.fastbidsheet"
}

/// `isTextualMime`: the exact eight-way predicate, empty mime included.
pub fn is_textual_mime(mime: &str) -> bool {
    mime.is_empty()
        || mime.starts_with("text/")
        || mime == "application/json"
        || mime.ends_with("+json")
        || mime == "application/xml"
        || mime.ends_with("+xml")
        || mime == "application/javascript"
        || mime == "application/x-javascript"
}

/// The tags whose text the extractor drops.
pub const SKIPPED_TAGS: [&str; 6] = ["script", "style", "noscript", "iframe", "object", "embed"];

/// Turndown's configuration, recorded for whoever ports the converter.
pub const TURNDOWN_HEADING_STYLE: &str = "atx";
pub const TURNDOWN_HR: &str = "---";
pub const TURNDOWN_BULLET_LIST_MARKER: &str = "-";
pub const TURNDOWN_CODE_BLOCK_STYLE: &str = "fenced";
pub const TURNDOWN_EM_DELIMITER: &str = "*";
/// `turndown.remove([...])`.
pub const TURNDOWN_REMOVED: [&str; 4] = ["script", "style", "meta", "link"];

/// `isCloudflareChallenge`, reduced to the two facts it reads.
pub fn is_cloudflare_challenge(status: u16, cf_mitigated: Option<&str>) -> bool {
    status == 403 && cf_mitigated == Some("challenge")
}

/// `assertHttpUrl`: the protocol check, on a string.
///
/// The source builds a `URL` first, so a malformed URL fails before the
/// protocol is even looked at. This checks the scheme and a non-empty host,
/// which covers the same inputs for the cases the tool can act on; a URL that
/// parses but has no host (`http:///path`) is rejected here and accepted-then-
/// failed-there by the source.
pub fn assert_http_url(url: &str) -> Result<(), &'static str> {
    let lower = url.to_lowercase();
    let rest = if let Some(rest) = lower.strip_prefix("http://") {
        rest
    } else if let Some(rest) = lower.strip_prefix("https://") {
        rest
    } else {
        return Err("URL must use http:// or https://");
    };
    let host: String = rest.chars().take_while(|c| !matches!(c, '/' | '?' | '#')).collect();
    if host.is_empty() {
        return Err("URL must use http:// or https://");
    }
    Ok(())
}

/// `is_html`: whether the response body is HTML at all.
pub fn is_html(content_type: &str) -> bool {
    content_type.contains("text/html")
}

/// `needs_conversion`: the dispatch inside `convert`.
///
/// `true` means the body must go through an HTML converter; the markdown path
/// needs turndown, the text path needs the extractor below. `false` means the
/// bytes are returned unchanged.
pub fn needs_conversion(content_type: &str, format: Format) -> bool {
    is_html(content_type) && format != Format::Html
}

/// `extractTextFromHTML`.
///
/// The skip counter is tag-agnostic on purpose: every skipped open and every
/// skipped close touches it, and the tag names are never matched against each
/// other. See the module header for what that means on malformed markup.
pub fn extract_text_from_html(html: &str) -> String {
    let mut text = String::new();
    let mut skip_depth = 0usize;
    let bytes: Vec<char> = html.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != '<' {
            let start = i;
            while i < bytes.len() && bytes[i] != '<' {
                i += 1;
            }
            if skip_depth == 0 {
                text.extend_from_slice(&bytes[start..i]);
            }
            continue;
        }
        // A comment runs to the closing arrow, whatever it contains.
        if bytes[i..].starts_with(&['<', '!', '-', '-']) {
            let mut j = i + 4;
            while j + 2 < bytes.len() && !(bytes[j] == '-' && bytes[j + 1] == '-' && bytes[j + 2] == '>') {
                j += 1;
            }
            i = (j + 3).min(bytes.len());
            continue;
        }
        let closing = bytes.get(i + 1) == Some(&'/');
        let name_start = if closing { i + 2 } else { i + 1 };
        let mut j = name_start;
        while j < bytes.len() && bytes[j] != '>' && !bytes[j].is_whitespace() && bytes[j] != '/' {
            j += 1;
        }
        if j >= bytes.len() {
            // No closing angle bracket: htmlparser2 would keep this as text.
            if skip_depth == 0 {
                text.extend_from_slice(&bytes[i..]);
            }
            break;
        }
        let name: String = bytes[name_start..j].iter().collect::<String>().to_lowercase();
        let self_closing = bytes[..j].iter().rev().find(|c| !c.is_whitespace()) == Some(&'/');
        if closing {
            if skip_depth > 0 {
                skip_depth -= 1;
            }
        } else if !self_closing && (skip_depth > 0 || SKIPPED_TAGS.contains(&name.as_str())) {
            skip_depth += 1;
        }
        i = j + 1;
    }
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_defaults_to_markdown_and_is_always_written() {
        let v: Input = serde_json::from_value(json!({ "url": "https://a" })).unwrap();
        assert_eq!(v.format, Format::Markdown);
        let back = serde_json::to_value(&v).unwrap();
        assert_eq!(back, json!({ "url": "https://a", "format": "markdown" }));
    }

    #[test]
    fn the_three_accept_headers_are_exact() {
        assert_eq!(
            accept_header(Format::Markdown),
            "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1"
        );
        assert_eq!(
            accept_header(Format::Text),
            "text/plain;q=1.0, text/markdown;q=0.9, text/html;q=0.8, */*;q=0.1"
        );
        assert_eq!(
            accept_header(Format::Html),
            "text/html;q=1.0, application/xhtml+xml;q=0.9, text/plain;q=0.8, text/markdown;q=0.7, */*;q=0.1"
        );
    }

    #[test]
    fn the_request_has_three_headers_in_order() {
        let h = headers(Format::Text, "agent/1");
        assert_eq!(h.len(), 3);
        assert_eq!(h[0], ("User-Agent", "agent/1".to_string()));
        assert_eq!(h[1], ("Accept", accept_header(Format::Text).to_string()));
        assert_eq!(h[2], ("Accept-Language", "en-US,en;q=0.9".to_string()));
    }

    #[test]
    fn the_browser_agent_names_windows_and_chrome_143() {
        assert!(BROWSER_USER_AGENT.contains("Windows NT 10.0"));
        assert!(BROWSER_USER_AGENT.contains("Chrome/143.0.0.0"));
        assert_eq!(RETRY_USER_AGENT, "opencode");
    }

    #[test]
    fn the_mime_is_stripped_of_its_parameters_and_lowercased() {
        assert_eq!(mime_from("text/HTML; charset=UTF-8"), "text/html");
        assert_eq!(mime_from("  application/json  "), "application/json");
        assert_eq!(mime_from(""), "");
        assert_eq!(mime_from(";x"), "");
    }

    #[test]
    fn an_image_is_refused_unless_it_is_one_of_the_two_exceptions() {
        assert!(is_image_attachment("image/png"));
        assert!(!is_image_attachment("image/svg+xml"), "svg is text-shaped");
        assert!(!is_image_attachment("image/vnd.fastbidsheet"));
        assert!(!is_image_attachment("text/html"));
    }

    #[test]
    fn the_textual_predicate_accepts_exactly_its_eight_shapes() {
        for mime in [
            "",
            "text/plain",
            "text/csv",
            "application/json",
            "application/vnd.api+json",
            "application/xml",
            "image/svg+xml+xml",
            "application/javascript",
            "application/x-javascript",
        ] {
            assert!(is_textual_mime(mime), "{}", mime);
        }
        for mime in ["image/png", "application/pdf", "application/octet-stream", "application/x-sh"] {
            assert!(!is_textual_mime(mime), "{}", mime);
        }
    }

    #[test]
    fn only_an_http_or_https_url_is_accepted() {
        assert!(assert_http_url("http://a").is_ok());
        assert!(assert_http_url("https://a/b?c=d").is_ok());
        assert!(assert_http_url("HTTPS://A").is_ok(), "the scheme is case insensitive");
        assert!(assert_http_url("ftp://a").is_err());
        assert!(assert_http_url("file:///etc/passwd").is_err());
        assert!(assert_http_url("https://").is_err(), "an empty host is refused");
        assert!(assert_http_url("a").is_err());
    }

    #[test]
    fn a_challenge_is_a_403_with_the_right_header() {
        assert!(is_cloudflare_challenge(403, Some("challenge")));
        assert!(!is_cloudflare_challenge(403, Some("other")));
        assert!(!is_cloudflare_challenge(403, None));
        assert!(!is_cloudflare_challenge(200, Some("challenge")));
    }

    #[test]
    fn the_timeout_is_bounded_at_both_ends() {
        let ok = Input { url: "u".into(), format: Format::Text, timeout: Some(MAX_TIMEOUT_SECONDS) };
        assert!(ok.validated().is_ok());
        assert!(Input { timeout: Some(0), ..ok.clone() }.validated().is_err());
        assert!(Input { timeout: Some(-1), ..ok.clone() }.validated().is_err());
        assert!(Input { timeout: Some(MAX_TIMEOUT_SECONDS + 1), ..ok.clone() }.validated().is_err());
        assert_eq!(Input { timeout: None, ..ok }.effective_timeout_seconds(), DEFAULT_TIMEOUT_SECONDS);
    }

    #[test]
    fn the_output_keeps_content_type_as_one_word() {
        let out = Output {
            url: "https://a".into(),
            content_type: "text/html".into(),
            format: Format::Markdown,
            output: "# t".into(),
        };
        let v = serde_json::to_value(&out).unwrap();
        assert_eq!(v["contentType"], json!("text/html"));
        assert!(v.get("content_type").is_none());
        assert_eq!(v["format"], json!("markdown"));
    }

    #[test]
    fn only_html_in_another_format_needs_conversion() {
        assert!(needs_conversion("text/html", Format::Markdown));
        assert!(needs_conversion("text/html", Format::Text));
        assert!(!needs_conversion("text/html", Format::Html), "html is passed through");
        assert!(!needs_conversion("application/json", Format::Markdown));
    }

    #[test]
    fn plain_text_survives_extraction() {
        assert_eq!(extract_text_from_html("hello"), "hello");
        assert_eq!(extract_text_from_html("  padded  "), "padded", "the result is trimmed");
    }

    #[test]
    fn tags_are_dropped_but_their_text_is_not() {
        assert_eq!(extract_text_from_html("<p>visible</p>"), "visible");
        assert_eq!(extract_text_from_html("<p>a<b>bold</b>c</p>"), "aboldc");
    }

    #[test]
    fn the_six_skipped_tags_drop_their_content() {
        for tag in SKIPPED_TAGS {
            assert_eq!(
                extract_text_from_html(&format!("<{0}>secret</{0}>shown", tag)),
                "shown",
                "{}",
                tag
            );
        }
    }

    #[test]
    fn a_skipped_tag_can_nest() {
        assert_eq!(
            extract_text_from_html("<object><embed>x</embed></object>after"),
            "after"
        );
    }

    #[test]
    fn the_counter_is_tag_agnostic_so_bad_markup_leaks() {
        // Two skipped opens, one close: the depth never returns to zero, so
        // the trailing text stays skipped. The source behaves the same way.
        assert_eq!(extract_text_from_html("<script><style>x</script>y"), "");
    }

    #[test]
    fn attributes_do_not_leak_into_the_text() {
        assert_eq!(extract_text_from_html("<a href=\"/x\" title=\"t\">link</a>"), "link");
    }

    #[test]
    fn comments_are_dropped_entirely() {
        assert_eq!(extract_text_from_html("a<!-- hidden -->b"), "ab");
    }

    #[test]
    fn a_self_closing_skipped_tag_does_not_open_a_region() {
        // `<embed/>` never closes, so counting it would swallow the rest.
        assert_eq!(extract_text_from_html("<embed/>visible"), "visible");
    }

    #[test]
    fn the_bounds_are_the_source_values() {
        assert_eq!(MAX_RESPONSE_BYTES, 5_242_880);
        assert_eq!(DEFAULT_TIMEOUT_SECONDS, 30);
        assert_eq!(MAX_TIMEOUT_SECONDS, 120);
        assert_eq!(SKIPPED_TAGS.len(), 6);
        assert_eq!(TURNDOWN_REMOVED.len(), 4);
    }

    #[test]
    fn the_turndown_options_are_recorded_not_guessed() {
        assert_eq!(TURNDOWN_HEADING_STYLE, "atx");
        assert_eq!(TURNDOWN_HR, "---");
        assert_eq!(TURNDOWN_BULLET_LIST_MARKER, "-");
        assert_eq!(TURNDOWN_CODE_BLOCK_STYLE, "fenced");
        assert_eq!(TURNDOWN_EM_DELIMITER, "*");
    }
}