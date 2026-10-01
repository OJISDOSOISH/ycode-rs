//! Port of the portable part of
//! `opencode/packages/core/src/location-mutation.ts`.
//!
//! This is where a tool call's target path is checked against the Location it
//! is allowed to touch, so the decision tree is security-relevant and is ported
//! whole.
//!
//! Not ported: `resolvePath`, which walks up the path asking the filesystem for
//! each ancestor's real path. That needs a real disk. What is ported is
//! everything above it, taking the resolved path as an input - which is exactly
//! the part that decides allow, refuse, or route to an external approval.
//!
//! The order of the checks is the contract, and reordering them changes what is
//! refused:
//!
//! 1. A RELATIVE path that leaves the Location is `relative_escape`. This comes
//!    before any filesystem work: a path that is obviously outside is refused
//!    without being resolved, so a symlink cannot be used to make an escaping
//!    relative path look internal.
//!
//! 2. A path that is lexically inside but whose CANONICAL form is not inside
//!    the canonical Location root is `location_escape`. This is the symlink
//!    check, and it is why the canonical root is compared rather than the
//!    lexical directory: `/base/link` lexically inside pointing at `/etc` is the
//!    attack this exists to catch.
//!
//! 3. A path that is outside becomes EXTERNAL rather than being refused - it
//!    still needs an `external_directory` approval, which is a separate check
//!    the caller performs.
//!
//! Two details worth stating because they are easy to get backwards:
//!
//! - the internal resource is the path RELATIVE to the canonical root, or `"."`
//!   when the target IS the root. `path.relative` returns `""` there, and `""`
//!   is falsy, so the `|| "."` is load-bearing: an empty permission resource
//!   would match nothing and silently deny every write to the root.
//!
//! - the external approval boundary is the target itself only when the caller
//!   asked for a directory AND the target resolved to a directory. Otherwise it
//!   is the CONTAINING directory, so approving a file grants the directory it
//!   sits in - which is the same granularity `join(dir, "*")` implies.

use serde::{Deserialize, Serialize};

/// `Kind`: what the caller expects to mutate. It selects the external approval
/// boundary; it does not validate the target type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Directory,
}

/// `ResolveInput`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ResolveInput {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
}

/// `PathError.reason`: the three ways a target can be refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathReason {
    /// A relative path that leaves the Location, refused before resolution.
    RelativeEscape,
    /// Lexically inside, canonically outside: a symlink out of the Location.
    LocationEscape,
    /// No existing ancestor directory, so the path cannot be canonicalised.
    NonDirectoryAncestor,
}

/// `PathError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathError {
    pub path: String,
    pub reason: PathReason,
}

/// `ExternalDirectoryAuthorization`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalDirectoryAuthorization {
    #[serde(rename = "action")]
    pub action: ExternalDirectoryAction,
    /// The canonical existing directory used as the approval boundary.
    pub directory: String,
    /// The `external_directory` permission resource.
    pub resource: String,
    pub save: String,
}

/// The only action an external boundary ever carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalDirectoryAction {
    #[serde(rename = "external_directory")]
    ExternalDirectory,
}

/// The permission rule an external boundary implies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    pub action: ExternalDirectoryAction,
    pub resources: Vec<String>,
    pub save: Vec<String>,
}

/// `externalDirectoryPermission`: the authorisation turned into a rule.
///
/// Both `resource` and `save` are wrapped in single-element arrays, and both
/// carry the SAME string - the `dir/*` glob. That is deliberate: the boundary is
/// the directory, so both the thing to check and the thing to remember are the
/// glob.
pub fn external_directory_permission(
    input: &ExternalDirectoryAuthorization,
) -> PermissionRule {
    PermissionRule {
        action: input.action,
        resources: vec![input.resource.clone()],
        save: vec![input.save.clone()],
    }
}

/// `Target`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    /// The canonical path, existing or missing below a canonical directory.
    pub canonical: String,
    /// The permission resource: Location-relative when internal, canonical when
    /// external.
    pub resource: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_directory: Option<ExternalDirectoryAuthorization>,
}

/// `slash`: backslashes to forward slashes, for every permission resource.
pub fn slash(value: &str) -> String {
    value.replace('\\', "/")
}

/// `fs.stat().type`, the eight possibilities the walk can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Directory,
    SymbolicLink,
    BlockDevice,
    CharacterDevice,
    Fifo,
    Socket,
    Unknown,
}

impl NodeKind {
    /// Whether this is a directory, the only case the walk branches on.
    pub fn is_directory(self) -> bool {
        matches!(self, NodeKind::Directory)
    }
}

/// `ResolvedPath`: what the filesystem walk produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPath {
    pub canonical: String,
    pub kind: Option<NodeKind>,
    /// The containing directory: the path itself when it is a directory.
    pub directory: String,
}

/// `path.dirname` for the slash-normalised paths this module handles.
pub fn dirname(path: &str) -> String {
    let normalised = slash(path);
    match normalised.rfind('/') {
        None => ".".to_string(),
        Some(0) => "/".to_string(),
        Some(index) => normalised[..index].to_string(),
    }
}

/// `path.isAbsolute`.
fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\') || {
        let bytes = path.as_bytes();
        bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\')
    }
}

/// `path.resolve(base, input)`, for an input that may be absolute already.
fn resolve_against(base: &str, input: &str) -> String {
    if is_absolute(input) {
        return normalise(&slash(input));
    }
    normalise(&format!("{}/{}", base.trim_end_matches(['/', '\\']), slash(input)))
}

/// `path.normalize` for an already absolute, slash-normalised path: collapse
/// `.` and resolve `..` lexically, keeping a leading separator.
fn normalise(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if matches!(out.last(), Some(&last) if last != "..") {
                    out.pop();
                } else if !absolute {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    let joined = out.join("/");
    if absolute {
        format!("/{}", joined)
    } else if joined.is_empty() {
        ".".to_string()
    } else {
        joined
    }
}

/// `FSUtil.contains`, reusing the implementation rather than restating it.
///
/// The platform is the HOST's, because the TypeScript `FSUtil.contains` calls
/// Node's `path.relative`, which is native: `path.win32.relative` on Windows,
/// `path.posix.relative` elsewhere. Passing `Platform::Posix` here regardless
/// of host was wrong in a way that mattered, because the two disagree about
/// what is rooted - `path.posix` does not treat a leading backslash as a root,
/// so a drive-rooted `C:\work` read as an ordinary relative name, and a sibling
/// path beside it came back "contained".
///
/// Still platform-naive, and deliberately so for now: `is_absolute` and
/// `resolve_against` above. They accept a leading backslash on either platform,
/// where `path.posix.isAbsolute("\\x")` is false, and `path.win32.resolve(base,
/// "\\x")` keeps the base's drive where this module would drop it. Both are
/// reachable only from a Windows-shaped input on a POSIX host, and fixing them
/// properly means threading a platform through `ResolveInput` and every test
/// that calls `resolve`. That is a larger change than it looks, and it is
/// recorded here rather than left to be rediscovered.
fn contains(parent: &str, child: &str) -> bool {
    super::fs_util::contains(parent, child, super::fs_util::Platform::host())
}

/// `path.relative`, reusing `config_plugin_path`'s implementation.
fn relative(from: &str, to: &str) -> String {
    super::config_plugin_path::relative(from, to)
}

/// `resolve`: the whole decision, with the filesystem step supplied.
///
/// `location_directory` is the lexical Location directory and `location_root`
/// its canonical form; they differ exactly when the Location itself sits behind
/// a symlink, which is why the TS compares the canonical path against the
/// canonical root rather than against the directory it started from.
pub fn resolve(
    input: &ResolveInput,
    location_directory: &str,
    location_root: &str,
    resolved: &ResolvedPath,
) -> Result<Target, PathError> {
    let relative_input = !is_absolute(&input.path);
    let absolute = resolve_against(location_directory, &input.path);
    let lexically_internal = contains(location_directory, &absolute);
    if relative_input && !lexically_internal {
        return Err(PathError {
            path: input.path.clone(),
            reason: PathReason::RelativeEscape,
        });
    }

    if lexically_internal && !contains(location_root, &resolved.canonical) {
        return Err(PathError {
            path: input.path.clone(),
            reason: PathReason::LocationEscape,
        });
    }

    let external = !lexically_internal;
    let resource = if external {
        slash(&resolved.canonical)
    } else {
        let inside = relative(location_root, &resolved.canonical);
        // `path.relative` answers "" for the root itself, and "" is falsy, so
        // the "." here is what makes a write to the root addressable at all.
        slash(if inside.is_empty() { "." } else { &inside })
    };

    let external_directory = if input.kind == Some(Kind::Directory)
        && resolved.kind.map(NodeKind::is_directory).unwrap_or(false)
    {
        resolved.canonical.clone()
    } else {
        resolved.directory.clone()
    };
    let external_resource = slash(&format!("{}/{}", external_directory.trim_end_matches('/'), "*"));

    Ok(Target {
        canonical: resolved.canonical.clone(),
        resource,
        external_directory: external.then(|| ExternalDirectoryAuthorization {
            action: ExternalDirectoryAction::ExternalDirectory,
            directory: external_directory,
            resource: external_resource.clone(),
            save: external_resource,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCATION: &str = "/work/project";
    const ROOT: &str = "/work/project";

    fn resolved(canonical: &str, kind: NodeKind) -> ResolvedPath {
        let directory = if kind.is_directory() { canonical.to_string() } else { dirname(canonical) };
        ResolvedPath { canonical: canonical.to_string(), kind: Some(kind), directory }
    }

    /// Resolves an input path the way the real service would, i.e. against the
    /// Location directory first, so the `canonical` handed to `resolve` is
    /// absolute. Passing the raw input as `canonical` would make every internal
    /// case look like a symlink out of the Location, because a relative
    /// canonical path is not contained by an absolute root.
    fn resolve_path(path: &str) -> Result<Target, PathError> {
        let absolute = resolve_against(LOCATION, path);
        resolve(
            &ResolveInput { path: path.into(), kind: None },
            LOCATION,
            ROOT,
            &resolved(&absolute, NodeKind::File),
        )
    }

    // --- the escapes ---

    #[test]
    fn a_relative_path_that_leaves_is_refused_before_resolution() {
        let err = resolve_path("../outside.txt").unwrap_err();
        assert_eq!(err.reason, PathReason::RelativeEscape);
        assert_eq!(err.path, "../outside.txt");
    }

    #[test]
    fn a_relative_path_that_climbs_far_out_is_still_refused() {
        assert_eq!(resolve_path("../../..").unwrap_err().reason, PathReason::RelativeEscape);
        assert_eq!(resolve_path("a/../../b").unwrap_err().reason, PathReason::RelativeEscape);
    }

    #[test]
    fn a_symlink_out_of_the_location_is_the_second_check() {
        // Lexically inside, canonically outside. The supplied resolved path is
        // outside the root while the input is inside the directory.
        let err = resolve(
            &ResolveInput { path: "link/passwd".into(), kind: None },
            LOCATION,
            ROOT,
            &resolved("/etc/passwd", NodeKind::File),
        )
        .unwrap_err();
        assert_eq!(err.reason, PathReason::LocationEscape, "a symlink out is not a relative escape");
    }

    #[test]
    fn the_two_escapes_are_distinguished() {
        assert_eq!(resolve_path("../x").unwrap_err().reason, PathReason::RelativeEscape);
        assert_ne!(PathReason::RelativeEscape, PathReason::LocationEscape);
    }

    #[test]
    fn the_error_carries_its_reason_in_snake_case() {
        let e = PathError { path: "p".into(), reason: PathReason::NonDirectoryAncestor };
        assert_eq!(
            serde_json::to_value(&e).unwrap()["reason"],
            serde_json::json!("non_directory_ancestor")
        );
        let e = PathError { path: "p".into(), reason: PathReason::RelativeEscape };
        assert_eq!(serde_json::to_value(&e).unwrap()["reason"], serde_json::json!("relative_escape"));
    }

    // --- internal targets ---

    #[test]
    fn an_internal_path_yields_a_relative_resource_and_no_external_approval() {
        let target = resolve_path("src/a.ts").unwrap();
        assert_eq!(target.resource, "src/a.ts");
        assert!(target.external_directory.is_none());
        assert_eq!(target.canonical, "/work/project/src/a.ts", "the canonical path is absolute");
    }

    #[test]
    fn the_location_root_itself_is_addressed_as_a_dot() {
        // `relative` answers "" here; without the ".", the resource would be
        // empty and would match nothing.
        let target = resolve_path(".").unwrap();
        assert_eq!(target.resource, ".", "an empty resource would deny every write to the root");
    }

    #[test]
    fn a_missing_file_below_a_canonical_directory_resolves() {
        let target = resolve(
            &ResolveInput { path: "new/dir/file.ts".into(), kind: Some(Kind::File) },
            LOCATION,
            ROOT,
            &ResolvedPath {
                canonical: "/work/project/new/dir/file.ts".into(),
                kind: None,
                directory: "/work/project/new/dir".into(),
            },
        )
        .unwrap();
        assert_eq!(target.resource, "new/dir/file.ts");
    }

    // --- external targets ---

    #[test]
    fn an_absolute_path_outside_becomes_external_rather_than_refused() {
        let err_input = ResolveInput { path: "/etc/hosts".into(), kind: None };
        let target = resolve(
            &err_input,
            LOCATION,
            ROOT,
            &resolved("/etc/hosts", NodeKind::File),
        )
        .unwrap();
        assert_eq!(target.resource, "/etc/hosts", "an external resource is the canonical path");
        let external = target.external_directory.expect("an external path needs an approval");
        assert_eq!(external.directory, "/etc");
        assert_eq!(external.resource, "/etc/*");
    }

    #[test]
    fn an_absolute_path_inside_needs_no_approval() {
        let target = resolve(
            &ResolveInput { path: "/work/project/a.ts".into(), kind: None },
            LOCATION,
            ROOT,
            &resolved("/work/project/a.ts", NodeKind::File),
        )
        .unwrap();
        assert!(target.external_directory.is_none());
        assert_eq!(target.resource, "a.ts");
    }

    #[test]
    fn the_external_boundary_is_the_directory_only_when_one_was_asked_for() {
        // kind = directory AND the target resolved to a directory.
        let target = resolve(
            &ResolveInput { path: "/srv/data".into(), kind: Some(Kind::Directory) },
            LOCATION,
            ROOT,
            &resolved("/srv/data", NodeKind::Directory),
        )
        .unwrap();
        assert_eq!(target.external_directory.unwrap().directory, "/srv/data");
    }

    #[test]
    fn asking_for_a_directory_does_not_change_the_boundary_when_it_is_a_file() {
        let target = resolve(
            &ResolveInput { path: "/srv/data.txt".into(), kind: Some(Kind::Directory) },
            LOCATION,
            ROOT,
            &resolved("/srv/data.txt", NodeKind::File),
        )
        .unwrap();
        assert_eq!(
            target.external_directory.unwrap().directory,
            "/srv",
            "a file falls back to its containing directory"
        );
    }

    #[test]
    fn a_missing_target_falls_back_to_its_resolved_containing_directory() {
        let target = resolve(
            &ResolveInput { path: "/srv/new/file.txt".into(), kind: Some(Kind::Directory) },
            LOCATION,
            ROOT,
            &ResolvedPath {
                canonical: "/srv/new/file.txt".into(),
                kind: None,
                directory: "/srv/new".into(),
            },
        )
        .unwrap();
        // Borrowed once: `unwrap()` on an `Option` moves it, so calling it twice
        // on the same field is a use after move. The error names the SECOND
        // use, seven characters away from the cause.
        let external = target.external_directory.as_ref().expect("an external path needs an approval");
        assert_eq!(external.directory, "/srv/new");
        assert_eq!(external.resource, "/srv/new/*");
    }

    #[test]
    fn the_external_resource_is_always_a_directory_glob() {
        let target = resolve(
            &ResolveInput { path: "/srv/x".into(), kind: None },
            LOCATION,
            ROOT,
            &resolved("/srv/x", NodeKind::File),
        )
        .unwrap();
        let external = target.external_directory.unwrap();
        assert!(external.resource.ends_with("/*"));
        assert_eq!(external.save, external.resource, "save carries the same glob");
    }

    #[test]
    fn the_authorisation_becomes_a_single_element_rule() {
        let authorization = ExternalDirectoryAuthorization {
            action: ExternalDirectoryAction::ExternalDirectory,
            directory: "/srv".into(),
            resource: "/srv/*".into(),
            save: "/srv/*".into(),
        };
        let rule = external_directory_permission(&authorization);
        assert_eq!(rule.resources, vec!["/srv/*".to_string()]);
        assert_eq!(rule.save, vec!["/srv/*".to_string()]);
        assert_eq!(
            serde_json::to_value(&rule).unwrap()["action"],
            serde_json::json!("external_directory")
        );
    }

    // --- path helpers ---

    #[test]
    fn backslashes_become_slashes_in_every_resource() {
        assert_eq!(slash("C:\\work\\a"), "C:/work/a");
        assert_eq!(slash("/work/a"), "/work/a");
    }

    #[test]
    fn dirname_follows_the_slash_normalised_shape() {
        assert_eq!(dirname("/a/b/c"), "/a/b");
        assert_eq!(dirname("/a"), "/");
        assert_eq!(dirname("a"), ".");
    }

    #[test]
    fn node_kind_knows_one_thing() {
        assert!(NodeKind::Directory.is_directory());
        assert!(!NodeKind::File.is_directory());
        assert!(!NodeKind::SymbolicLink.is_directory());
        assert!(!NodeKind::Unknown.is_directory());
    }

    #[test]
    fn kind_serialises_in_lower_case() {
        assert_eq!(serde_json::to_value(Kind::File).unwrap(), serde_json::json!("file"));
        assert_eq!(serde_json::to_value(Kind::Directory).unwrap(), serde_json::json!("directory"));
    }

    #[test]
    fn the_input_omits_an_absent_kind() {
        let v = serde_json::to_value(ResolveInput { path: "a".into(), kind: None }).unwrap();
        assert_eq!(v, serde_json::json!({ "path": "a" }));
    }
}