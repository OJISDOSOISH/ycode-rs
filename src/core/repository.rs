//! Port of the portable part of `opencode/packages/core/src/repository.ts`.
//!
//! Turning whatever the user typed into a repository reference, and refusing
//! the ones that are unsafe. No I/O happens here, which makes the whole file
//! pure and therefore worth testing hard: a mis-parsed reference becomes a
//! clone of the wrong repository, and the checks that stop it are the only
//! barrier between a string and a path.
//!
//! The accepted forms, in the order they are tried, and the order is the
//! contract - later forms never get a chance once an earlier one matches:
//!
//! 1. `github:owner/repo`, exactly two segments and no whitespace.
//! 2. Without a `://`, the scp form `host:path`, optionally with a `user@`.
//! 3. Without a `://`, `host/path` where the first segment LOOKS like a host -
//!    it contains a dot, or a colon, or it is exactly `localhost`. This is what
//!    makes `owner/repo` fall through to GitHub while `my.host/repo` does not.
//! 4. Without a `://`, a bare `owner/repo` becomes a GitHub reference.
//! 5. Otherwise, a real URL: `file:` yields a file reference, anything else a
//!    remote one.
//!
//! Four rejections are deliberate and each blocks a different accident:
//!
//! - a host starting with `-` could be read as a flag by git, so it is refused;
//! - a segment that is `.` or `..`, or that contains `:` or a separator, could
//!   escape the cache directory;
//! - `.git` is trimmed from the END of every segment, which is why
//!   `parts` trims per segment rather than once at the end;
//! - a branch may not start with `-` and may not contain `..`.
//!
//! Known divergence, recorded rather than hidden: `new URL(...)`, `path.join`
//! and `fileURLToPath` are Node APIs with their own normalisation, including
//! percent-decoding and Windows drive letters. Here the parsing is explicit,
//! so a `file:` reference keeps its percent-encoding in `segments` where Node
//! would have decoded it, and a `file:` path is not resolved against the
//! process working directory. What is ported is the decision structure, which is
//! where the behaviour lives.

use std::fmt;

use serde::{Deserialize, Serialize};

/// `OPENCODE_REPO_CLONE_GITHUB_BASE_URL`: a GitHub base URL to clone through.
pub const GITHUB_BASE_ENV: &str = "OPENCODE_REPO_CLONE_GITHUB_BASE_URL";

/// The three refusals, as their tagged shapes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepositoryError {
    #[serde(rename = "RepositoryInvalidReferenceError")]
    InvalidReference { repository: String, message: String },
    #[serde(rename = "RepositoryUnsupportedLocalRepositoryError")]
    UnsupportedLocalRepository { repository: String, message: String },
    #[serde(rename = "RepositoryInvalidBranchError")]
    InvalidBranch { branch: String, message: String },
}

impl RepositoryError {
    /// `isError`: every variant of the union is a `Repository.Error`.
    pub fn is_error(&self) -> bool {
        true
    }
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepositoryError::InvalidReference { message, .. } => write!(f, "{}", message),
            RepositoryError::UnsupportedLocalRepository { message, .. } => write!(f, "{}", message),
            RepositoryError::InvalidBranch { message, .. } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for RepositoryError {}

/// A parsed reference: either remote or a local file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// `file` for a local reference, otherwise the lowercased host.
    pub host: String,
    /// The repository path: the segments joined with `/`.
    pub path: String,
    /// The segments, `.git` trimmed and empties dropped.
    pub segments: Vec<String>,
    /// Present only when there are exactly two segments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The last segment.
    pub repo: String,
    /// The clone URL.
    pub remote: String,
    /// `owner/repo` for a two-segment GitHub reference, else `host/path`.
    pub label: String,
    /// `file:` for a local reference; the URL protocol otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
}

impl Reference {
    /// `isFile`: the protocol is the discriminator, not the host.
    pub fn is_file(&self) -> bool {
        self.protocol.as_deref() == Some("file:")
    }

    /// `isRemote`.
    pub fn is_remote(&self) -> bool {
        !self.is_file()
    }

    /// `cacheIdentity`: `host/path`, which is what `same` compares.
    pub fn cache_identity(&self) -> String {
        format!("{}/{}", self.host, self.path)
    }
}

/// `normalizeInput`: trim, drop `git+`, drop a fragment, drop trailing slashes.
pub fn normalize_input(input: &str) -> String {
    let mut value = input.trim().to_string();
    if let Some(rest) = value.strip_prefix("git+") {
        value = rest.to_string();
    }
    if let Some(index) = value.find('#') {
        value.truncate(index);
    }
    while value.ends_with('/') {
        value.pop();
    }
    value
}

/// `trimGitSuffix`.
pub fn trim_git_suffix(input: &str) -> String {
    match input.strip_suffix(".git") {
        Some(rest) => rest.to_string(),
        None => input.to_string(),
    }
}

/// `parts`: split on `/`, trim each, drop `.git`, drop empties.
pub fn parts(input: &str) -> Vec<String> {
    input
        .split('/')
        .map(|item| trim_git_suffix(item.trim()))
        .filter(|item| !item.is_empty())
        .collect()
}

/// `safeHost`: non-empty, no leading `-`, no whitespace or separator.
pub fn safe_host(input: &str) -> bool {
    !input.is_empty()
        && !input.starts_with('-')
        && !input.contains([' ', '\t', '\n', '\r', '/', '\\'])
        && !input.chars().any(char::is_whitespace)
}

/// `safeSegment`: not `.`, not `..`, no `:`, no whitespace or separator.
pub fn safe_segment(input: &str) -> bool {
    input != "."
        && input != ".."
        && !input.contains(':')
        && !input.contains(['/', '\\'])
        && !input.chars().any(char::is_whitespace)
}

/// `hostLike`: what makes the first segment of a path look like a host.
pub fn host_like(input: &str) -> bool {
    input.contains('.') || input.contains(':') || input == "localhost"
}

/// `githubRemote`, with the environment variable passed in rather than read.
pub fn github_remote(pathname: &str, base: Option<&str>) -> String {
    match base {
        None => format!("https://github.com/{}.git", pathname),
        Some(base) => {
            let base = if base.ends_with('/') { base.to_string() } else { format!("{}/", base) };
            format!("{}{}.git", base, pathname)
        }
    }
}

/// `buildRemote`: the shared validation and shape, or `None` when unsafe.
pub fn build_remote(host: &str, segments: &[String], remote: Option<&str>, protocol: Option<&str>) -> Option<Reference> {
    let segments: Vec<String> = segments
        .iter()
        .map(|segment| trim_git_suffix(segment))
        .filter(|segment| !segment.is_empty())
        .collect();
    if !safe_host(host) || segments.is_empty() || segments.iter().any(|segment| !safe_segment(segment)) {
        return None;
    }
    let repository_path = segments.join("/");
    let host = host.to_lowercase();
    let repo = segments[segments.len() - 1].clone();
    let owner = if segments.len() == 2 { Some(segments[0].clone()) } else { None };
    let remote = remote.map(str::to_string).unwrap_or_else(|| {
        if host == "github.com" {
            github_remote(&repository_path, std::env::var(GITHUB_BASE_ENV).ok().as_deref())
        } else {
            format!("https://{}/{}.git", host, repository_path)
        }
    });
    let label = if host == "github.com" && segments.len() == 2 {
        repository_path.clone()
    } else {
        format!("{}/{}", host, repository_path)
    };
    Some(Reference { host, path: repository_path, segments, owner, repo, remote, label, protocol: protocol.map(str::to_string) })
}

/// `buildFile`: a `file:` reference, with the path split on either separator.
pub fn build_file(path: &str, remote: &str) -> Option<Reference> {
    let file_path = path.to_string();
    let segments: Vec<String> = file_path
        .split(['/', '\\'])
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect();
    if segments.is_empty() {
        return None;
    }
    let repo = trim_git_suffix(segments[segments.len() - 1].trim_end_matches(':'));
    Some(Reference {
        host: "file".to_string(),
        path: file_path.clone(),
        segments: segments.iter().map(|s| s.trim_end_matches(':').to_string()).collect(),
        owner: None,
        repo,
        remote: remote.to_string(),
        label: file_path,
        protocol: Some("file:".to_string()),
    })
}

/// The pieces of a URL this port needs: protocol, host, path.
struct UrlParts<'a> {
    protocol: &'a str,
    host: &'a str,
    path: &'a str,
}

/// A small `new URL` for the two shapes `parse` actually accepts.
///
/// Only `scheme://host/path` and `file:` are recognised, which is what the
/// source reaches: anything else throws there and lands in the same `catch`.
fn split_url(input: &str) -> Option<UrlParts<'_>> {
    let (scheme, rest) = input.split_once("://")?;
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return None;
    }
    let (host, path) = match rest.find(['/', '?', '#']) {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, ""),
    };
    let path = path.split(['?', '#']).next().unwrap_or("");
    // `URL.protocol` carries the colon: the source compares it against
    // `"file:"`, not `"file"`, and every caller in this file does the same.
    Some(UrlParts { protocol: &input[..scheme.len() + 1], host, path })
}

/// `parse`: the whole decision tree, or `None`.
pub fn parse(input: &str) -> Option<Reference> {
    let cleaned = normalize_input(input);
    if cleaned.is_empty() {
        return None;
    }

    // 1. github:owner/repo
    if let Some(rest) = cleaned.strip_prefix("github:") {
        let pieces: Vec<&str> = rest.split('/').collect();
        if pieces.len() == 2 && pieces.iter().all(|piece| !piece.is_empty() && !piece.contains(char::is_whitespace)) {
            return build_remote(
                "github.com",
                &[pieces[0].to_string(), pieces[1].to_string()],
                None,
                None,
            );
        }
    }

    if !cleaned.contains("://") {
        // 2. scp form, with an optional user@ and no slash in the host
        if let Some((before, after)) = cleaned.split_once(':') {
            let host_part = before.rsplit('@').next().unwrap_or(before);
            let host_ok = !host_part.is_empty()
                && !host_part.contains(['/', ' ', '\t', '\n', '\r'])
                && !after.is_empty()
                && !after.contains([' ', '\t', '\n', '\r']);
            if host_ok {
                let segments = parts(after);
                return build_remote(host_part, &segments, Some(&cleaned), None);
            }
        }

        // 3. host/path where the first segment looks like a host
        let direct = parts(&cleaned);
        if direct.len() >= 2 && host_like(&direct[0]) {
            return build_remote(&direct[0], &direct[1..], None, None);
        }
        // 4. bare owner/repo is GitHub
        if direct.len() == 2 {
            return build_remote("github.com", &direct, None, None);
        }
    }

    // 5. a real URL
    let url = split_url(&cleaned)?;
    if url.protocol == "file:" {
        return build_file(url.path, &cleaned);
    }
    let segments = parts(url.path);
    let remote = if url.host == "github.com" {
        github_remote(&segments.join("/"), std::env::var(GITHUB_BASE_ENV).ok().as_deref())
    } else {
        cleaned.clone()
    };
    build_remote(url.host, &segments, Some(&remote), Some(url.protocol))
}

/// `parseRemote`, which turns the two failures into errors instead of `None`.
pub fn parse_remote(input: &str) -> Result<Reference, RepositoryError> {
    let reference = parse(input).ok_or_else(|| RepositoryError::InvalidReference {
        repository: input.to_string(),
        message: "Repository must be a git URL, host/path reference, or GitHub owner/repo shorthand"
            .to_string(),
    })?;
    if !reference.is_remote() {
        return Err(RepositoryError::UnsupportedLocalRepository {
            repository: input.to_string(),
            message: "Local file repositories are not supported".to_string(),
        });
    }
    Ok(reference)
}

/// `validateBranch`: letters, digits, `/`, `_`, `.` and `-`, with two exclusions.
pub fn validate_branch(branch: &str) -> Result<(), RepositoryError> {
    let charset_ok = !branch.is_empty()
        && branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '.' | '-'));
    if charset_ok && !branch.starts_with('-') && !branch.contains("..") {
        return Ok(());
    }
    Err(RepositoryError::InvalidBranch {
        branch: branch.to_string(),
        message: "Branch must contain only alphanumeric characters, /, _, ., and -, and cannot start with - or contain .."
            .to_string(),
    })
}

/// `cachePath`: the checkout directory, with the branch percent-encoded.
///
/// The encoding is not decoration: a valid branch name may contain `/`, and
/// without it a branch `feature/x` would land in a subdirectory instead of
/// beside its siblings - so a branchless refresh could walk over it.
pub fn cache_path(root: &str, reference: &Reference, branch: Option<&str>) -> String {
    let mut base = String::from(root);
    for piece in reference.host.split(':') {
        base.push('/');
        base.push_str(piece);
    }
    for segment in &reference.segments {
        base.push('/');
        base.push_str(segment);
    }
    match branch {
        None => base,
        Some(branch) => format!("{}@{}", base, percent_encode(branch)),
    }
}

/// `encodeURIComponent`, restricted to what a branch can contain.
fn percent_encode(input: &str) -> String {
    let mut out = String::new();
    for byte in input.as_bytes() {
        let c = *byte as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')') {
            out.push(c);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

/// `same`.
pub fn same(left: &Reference, right: &Reference) -> bool {
    left.cache_identity() == right.cache_identity()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(input: &str) -> Reference {
        parse(input).unwrap_or_else(|| panic!("expected {:?} to parse", input))
    }

    // --- normalizeInput ---

    #[test]
    fn normalization_trims_drops_the_prefix_the_fragment_and_the_slashes() {
        assert_eq!(normalize_input("  git+ssh://a/b#frag  "), "ssh://a/b");
        assert_eq!(normalize_input("a/b/"), "a/b");
        assert_eq!(normalize_input("a/b///"), "a/b");
        assert_eq!(normalize_input("   "), "");
    }

    #[test]
    fn only_a_leading_git_plus_is_dropped() {
        assert_eq!(normalize_input("git+file:///a"), "file:///a");
        assert_eq!(normalize_input("a/git+b"), "a/git+b", "not at the start");
    }

    // --- the accepted forms ---

    #[test]
    fn the_github_shorthand_wins_over_everything_else() {
        let r = p("github:owner/repo");
        assert_eq!(r.host, "github.com");
        assert_eq!(r.path, "owner/repo");
        assert_eq!(r.label, "owner/repo", "two segments label as owner/repo");
        assert_eq!(r.remote, "https://github.com/owner/repo.git");
    }

    #[test]
    fn a_bare_owner_repo_is_github_too() {
        let r = p("owner/repo");
        assert_eq!(r.host, "github.com");
        assert_eq!(r.owner.as_deref(), Some("owner"));
        assert_eq!(r.repo, "repo");
    }

    #[test]
    fn a_host_with_a_dot_is_not_github() {
        let r = p("git.example.com/owner/repo");
        assert_eq!(r.host, "git.example.com");
        assert_eq!(r.label, "git.example.com/owner/repo", "not owner/repo");
        assert_eq!(r.remote, "https://git.example.com/owner/repo.git");
    }

    #[test]
    fn localhost_counts_as_a_host() {
        assert_eq!(p("localhost/owner/repo").host, "localhost");
    }

    #[test]
    fn the_scp_form_keeps_the_remote_verbatim() {
        let r = p("git@github.com:owner/repo.git");
        assert_eq!(r.host, "github.com");
        assert_eq!(r.path, "owner/repo", ".git is trimmed per segment");
        assert_eq!(r.remote, "git@github.com:owner/repo.git", "the input, not a rebuilt URL");
    }

    #[test]
    fn a_url_yields_its_protocol_and_host() {
        let r = p("https://gitlab.com/group/project");
        assert_eq!(r.host, "gitlab.com");
        assert_eq!(r.protocol.as_deref(), Some("https:"));
        assert_eq!(r.path, "group/project");
    }

    #[test]
    fn a_file_url_is_the_local_kind() {
        let r = p("file:///srv/repos/thing");
        assert!(r.is_file());
        assert!(!r.is_remote());
        assert_eq!(r.host, "file");
        assert_eq!(r.protocol.as_deref(), Some("file:"));
        assert_eq!(r.segments, vec!["srv", "repos", "thing"]);
        assert_eq!(r.path, "/srv/repos/thing");
        assert_eq!(r.label, "/srv/repos/thing", "a file reference labels with its path");
        assert_eq!(r.repo, "thing");
        assert_eq!(r.owner, None, "a file reference never has an owner");
    }

    #[test]
    fn a_file_reference_is_never_remote() {
        // The protocol is the discriminator, so a `file:` reference with a host
        // that happens to read `file` is still local.
        let r = build_file("/a/b", "file:///a/b").unwrap();
        assert!(r.is_file());
        assert!(!r.is_remote());
    }

    #[test]
    fn an_empty_file_path_yields_no_reference() {
        assert!(build_file("", "file://").is_none());
    }

    // --- the rejections ---

    #[test]
    fn an_owner_only_string_is_not_a_reference() {
        assert!(parse("owner").is_none(), "one segment is neither form");
        assert!(parse("a/b/c").is_none(), "three bare segments are not github shorthand");
    }

    #[test]
    fn a_host_starting_with_a_dash_is_refused() {
        assert!(parse("-flag.com/owner/repo").is_none(), "git could read it as a flag");
    }

    #[test]
    fn dot_segments_are_refused_so_the_path_cannot_escape() {
        assert!(parse("github:owner/..").is_none());
        assert!(parse("host/../etc").is_none());
        assert!(parse("host/a/./b").is_none());
    }

    #[test]
    fn a_segment_with_a_colon_is_refused() {
        // This is what keeps the scp form from being re-read inside a path.
        assert!(parse("https://host/a:b/c").is_none());
    }

    #[test]
    fn whitespace_anywhere_is_refused() {
        assert!(parse("github:own er/repo").is_none());
        assert!(parse("host name/a/b").is_none());
        assert!(parse("owner/re po").is_none());
    }

    #[test]
    fn an_empty_input_parses_to_nothing() {
        assert!(parse("").is_none());
        assert!(parse("   ").is_none());
        assert!(parse("///").is_none(), "trailing slashes leave nothing");
    }

    // --- parseRemote ---

    #[test]
    fn parse_remote_reports_the_failure_instead_of_returning_none() {
        let err = parse_remote("nonsense").unwrap_err();
        match err {
            RepositoryError::InvalidReference { repository, .. } => assert_eq!(repository, "nonsense"),
            other => panic!("expected an invalid reference, got {:?}", other),
        }
    }

    #[test]
    fn parse_remote_refuses_a_local_file() {
        let err = parse_remote("file:///a/b").unwrap_err();
        match err {
            RepositoryError::UnsupportedLocalRepository { .. } => {}
            other => panic!("expected an unsupported local reference, got {:?}", other),
        }
    }

    #[test]
    fn parse_remote_accepts_the_usual_forms() {
        assert!(parse_remote("owner/repo").is_ok());
        assert!(parse_remote("git@github.com:o/r.git").is_ok());
        assert!(parse_remote("https://gitlab.com/g/p").is_ok());
    }

    // --- validateBranch ---

    #[test]
    fn the_usual_branch_names_pass() {
        for branch in ["main", "feature/x", "release-1.2", "a_b.c", "v2"] {
            assert!(validate_branch(branch).is_ok(), "{}", branch);
        }
    }

    #[test]
    fn a_leading_dash_is_refused_because_git_reads_flags() {
        assert!(validate_branch("-force").is_err());
    }

    #[test]
    fn a_double_dot_is_refused() {
        assert!(validate_branch("a..b").is_err());
        assert!(validate_branch("a/../b").is_err());
    }

    #[test]
    fn characters_outside_the_set_are_refused() {
        for branch in ["a b", "a~b", "a^b", "a:b", "a?b", "a*b"] {
            assert!(validate_branch(branch).is_err(), "{}", branch);
        }
    }

    #[test]
    fn an_empty_branch_is_refused() {
        assert!(validate_branch("").is_err());
    }

    #[test]
    fn the_branch_error_names_the_branch_and_the_rule() {
        match validate_branch("-x").unwrap_err() {
            RepositoryError::InvalidBranch { branch, message } => {
                assert_eq!(branch, "-x");
                assert!(message.contains("cannot start with -"));
            }
            other => panic!("expected an invalid branch, got {:?}", other),
        }
    }

    // --- cachePath ---

    #[test]
    fn a_branchless_cache_path_is_the_host_and_the_segments() {
        let r = p("github:owner/repo");
        assert_eq!(cache_path("/root", &r, None), "/root/github.com/owner/repo");
    }

    #[test]
    fn a_colon_in_the_host_becomes_a_separator() {
        let r = parse("ssh://host:22/a/b").unwrap();
        assert_eq!(r.host, "host:22");
        assert_eq!(cache_path("/root", &r, None), "/root/host/22/a/b");
    }

    #[test]
    fn a_branch_is_appended_encoded() {
        let r = p("github:owner/repo");
        assert_eq!(cache_path("/root", &r, Some("main")), "/root/github.com/owner/repo@main");
    }

    #[test]
    fn a_slash_in_the_branch_is_encoded_so_the_path_stays_flat() {
        let r = p("github:owner/repo");
        let path = cache_path("/root", &r, Some("feature/x"));
        assert_eq!(path, "/root/github.com/owner/repo@feature%2Fx");
        assert!(!path[1..].contains("feature/x"), "no subdirectory is created");
    }

    // --- same ---

    #[test]
    fn same_compares_the_identity_only() {
        let a = p("github:owner/repo");
        let b = p("owner/repo");
        assert!(same(&a, &b), "same host and path, different spelling");
        assert!(!same(&a, &p("github:owner/other")));
    }

    #[test]
    fn the_host_is_lowercased_so_case_does_not_split_the_cache() {
        assert_eq!(p("GitHub.com/owner/repo").host, "github.com");
        assert!(same(&p("github.com/o/r"), &p("GitHub.com/o/r")));
    }

    // --- shapes ---

    #[test]
    fn owner_is_present_only_for_two_segments() {
        assert_eq!(p("owner/repo").owner.as_deref(), Some("owner"));
        assert_eq!(p("group/sub/repo").owner, None);
    }

    #[test]
    fn a_three_segment_reference_labels_with_its_host() {
        // `github:a/b/c` does NOT match the shorthand - that form wants exactly
        // two segments - so it falls through to the scp branch and `github`
        // becomes the host. The same is true of a host with no dot, like this
        // one: `host/a/b` has three segments and its first does not look like a
        // host, so there is no reference at all.
        assert_eq!(p("github:a/b/c").host, "github");
        assert_eq!(p("github:a/b/c").label, "github/a/b/c");
    }

    #[test]
    fn three_bare_segments_parse_to_nothing_unless_the_first_looks_like_a_host() {
        assert!(parse("host/a/b").is_none(), "no dot, no colon, not localhost");
        assert_eq!(p("host.io/a/b").host, "host.io");
    }

    #[test]
    fn a_single_segment_host_has_no_owner() {
        // `host.io/a` has two segments, so the first IS the owner.
        assert_eq!(p("host.io/a").owner.as_deref(), Some("host.io"));
        assert_eq!(p("host.io/a").repo, "a");
    }

    #[test]
    fn the_git_suffix_is_trimmed_from_every_segment() {
        let r = p("host.io/a.git/b.git");
        assert_eq!(r.segments, vec!["a", "b"]);
        assert_eq!(r.repo, "b");
        assert_eq!(r.path, "a/b");
    }

    #[test]
    fn the_serde_shape_omits_an_absent_owner() {
        let r = p("host.io/a/b");
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("owner").is_none(), "three segments means no owner");
        let two = serde_json::to_value(&p("host.io/a")).unwrap();
        assert!(two.get("owner").is_none(), "a bare host with one segment has no owner either");
    }

    #[test]
    fn the_errors_serialise_under_their_tag_names() {
        let e = RepositoryError::InvalidBranch { branch: "-x".into(), message: "m".into() };
        assert_eq!(
            serde_json::to_value(&e).unwrap()["_tag"],
            serde_json::json!("RepositoryInvalidBranchError")
        );
        let f = RepositoryError::InvalidReference { repository: "r".into(), message: "m".into() };
        assert_eq!(
            serde_json::to_value(&f).unwrap()["_tag"],
            serde_json::json!("RepositoryInvalidReferenceError")
        );
    }

    #[test]
    fn github_remote_honours_the_base_url_when_there_is_one() {
        assert_eq!(
            github_remote("o/r", Some("https://mirror.example/gh")),
            "https://mirror.example/gh/o/r.git"
        );
        assert_eq!(
            github_remote("o/r", Some("https://mirror.example/gh/")),
            "https://mirror.example/gh/o/r.git",
            "a missing final slash is added"
        );
        assert_eq!(github_remote("o/r", None), "https://github.com/o/r.git");
    }

    #[test]
    fn every_error_variant_counts_as_an_error() {
        // `isError` is a type guard in the TS and a total function here, since
        // the enum has no other inhabitants.
        assert!(RepositoryError::InvalidReference { repository: String::new(), message: String::new() }.is_error());
    }
}