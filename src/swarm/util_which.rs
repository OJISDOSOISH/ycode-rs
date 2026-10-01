//! Port of `opencode/packages/core/src/util/which.ts`.
//!
//! The TypeScript source is fourteen lines and delegates the real work to the
//! npm package `which`, which touches the file system. The port therefore splits
//! the file in two halves, and only one of them is ported:
//!
//! * **Resolution logic, ported in full and pure.** Picking `PATH` out of the
//!   two environment sources, picking `PATHEXT`, concatenating
//!   `Global.Path.bin` onto the search path, splitting that path on the Windows
//!   delimiter, expanding `PATHEXT` into candidate file names, and walking the
//!   directories in order. Every one of those functions takes a slice of data
//!   and returns a value. No file system, no process, no clock, no randomness.
//!
//! * **The side effect, injected, not performed.** The `which` package asks the
//!   operating system "does this path exist and is it an executable file". That
//!   question is the `probe` closure of [`which`] and of [`search_dirs`]. A
//!   caller on the target would pass a probe built on
//!   `std::fs::metadata(path)` returning true when the entry is a file and not a
//!   directory. This module never builds such a probe and never touches the
//!   file system, so every test in this file is pure and instantaneous.
//!
//! The project targets Windows only, so the POSIX branch of `which` is not
//! ported. The path delimiter is therefore hard-coded to `;`, the separator to
//! `\`, and the default `PATHEXT` to `.COM;.EXE;.BAT;.CMD`.
//!
//! ## Truthiness and nullity: two different chains, kept apart
//!
//! The source mixes the two operators and the difference is observable:
//!
//! ```text
//! const base = env?.PATH ?? env?.Path ?? process.env.PATH ?? process.env.Path ?? ""
//! const full = base ? base + path.delimiter + Global.Path.bin : Global.Path.bin
//! ```
//!
//! `??` yields the left operand only when it is `null` or `undefined`, so an
//! **empty string survives** the chain. A caller that passes `{ PATH: "" }` gets
//! an empty `base`, not the process `PATH`.
//!
//! `?:` tests truthiness, so an **empty string disappears**. That empty `base`
//! produces `full = Global.Path.bin` with no leading delimiter, not `";" +
//! Global.Path.bin`.
//!
//! A single helper cannot express both. The port keeps [`select_path`]
//! (nullity, `??`, four steps then `""`) and [`build_search_path`] (truthiness,
//! `?`) as two separate functions, and two tests pin the difference.
//!
//! ## Trap 1: the key names are capitalised
//!
//! The environment keys are `PATH`, `Path`, `PATHEXT`, `PathExt`. `Path` is not
//! `PATH`, and `PathExt` is not `PATHEXT`. A mistake there is invisible to the
//! compiler: `lookup_exact` compares byte for byte, so a wrong key silently
//! falls through to the next step of the chain and picks up the process
//! environment instead of the caller's value. Two facts make this worse than it
//! looks:
//!
//! * The real Windows process environment is case-insensitive, so `process.env`
//!   would accept any spelling. An ordinary JavaScript object passed as `env` is
//!   case-sensitive. The port models the case-sensitive lookup, because that is
//!   the branch the caller controls.
//! * The `which` option key is `pathExt` in camel case, not `path_ext` nor
//!   `PATHEXT`. [`WhichCall`] carries `#[serde(rename_all = "camelCase")]` and a
//!   test asserts the exact key set `nothrow`, `path`, `pathExt`.
//!
//! ## Known risk: byte-index slicing can panic at run time
//!
//! `&s[..n]` cuts at a byte offset. If `n` lands inside a multi-byte character
//! the slice panics, and the compiler says nothing. Nothing in this file
//! indexes a string by byte offset, and nothing will: every split goes through
//! [`split_path_list`], which uses the character-aware `str::split`, and
//! extension matching goes through [`ends_with_ignore_ascii_case`], which walks
//! [`str::chars`] in reverse instead of subtracting lengths. Two tests use
//! multi-byte input to prove neither can panic.
//!
//! ## What is not ported
//!
//! * The file system probe itself, as described above.
//! * `path.win32.normalize` in full. [`join_dir`] reproduces the join and the
//!   collapsing of a duplicated trailing separator, which covers absolute `PATH`
//!   entries, but not trailing dots, `..` segments, UNC roots or device paths.
//! * The POSIX branch, by design.
//! * `which` also re-reads `process.env.PATHEXT` when the caller passes no
//!   `pathExt`. That fallback is modelled by [`effective_path_ext`], which takes
//!   the environment as data instead of reading the real one.

use serde::{Deserialize, Serialize};

/// Path separator of the `which` package on Windows.
pub const DELIMITER: char = ';';

/// Separator between a directory and a file name on Windows, as `path.join` emits.
pub const SEPARATOR: char = '\\';

/// `windowsDefaultPathExt` of the `which` package, used when neither the caller
/// nor the process environment provides a `PATHEXT`.
pub const DEFAULT_PATH_EXT: &str = ".COM;.EXE;.BAT;.CMD";

/// The option object the source passes to `whichPkg.sync`.
///
/// The field names reproduce the TypeScript literal
/// `{ nothrow: true, path: full, pathExt: ... }`. `path_ext` is renamed to
/// `pathExt` by the container attribute, so the serialised key set is exactly
/// `nothrow`, `path`, `pathExt`. `nothrow` is the reason the source can end with
/// a `typeof result === "string"` test at all: without it, `which` throws
/// instead of returning `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhichCall {
    /// TS key `nothrow`. The source always passes `true`, so a miss is a value
    /// and never an exception.
    pub nothrow: bool,
    /// TS key `path`. The already concatenated search path, base then
    /// `Global.Path.bin`, separated by [`DELIMITER`].
    pub path: String,
    /// TS key `pathExt`. `None` means `undefined`, which is what the source
    /// passes when none of the four looked-up keys exists. `Some("")` is a
    /// different thing and is preserved, see [`effective_path_ext`].
    #[serde(default)]
    pub path_ext: Option<String>,
}

/// An environment seen as data: the same key can appear more than once, and the
/// lookup is byte-exact.
pub type EnvMap = [(String, String)];

/// Looks a key up by exact bytes, like JavaScript property access on a plain
/// object.
///
/// The first match wins. Comparison is case-sensitive on purpose, see the trap
/// section of the module documentation.
///
/// ```
/// use ycode::swarm::util_which::{lookup_exact, EnvMap};
///
/// let env: EnvMap = [(String::from("Path"), String::from("C:\\a"))];
/// assert_eq!(lookup_exact(&env, "Path"), Some("C:\\a"));
/// // "PATH" is not "Path", so the lookup falls through to the next step.
/// assert_eq!(lookup_exact(&env, "PATH"), None);
/// ```
pub fn lookup_exact<'a>(env: &'a [(String, String)], key: &str) -> Option<&'a str> {
    env.iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.as_str())
}

/// The `??` chain that produces `base` in the source.
///
/// Order, exactly: `env.PATH`, `env.Path`, `process.env.PATH`,
/// `process.env.Path`, then the `""` default. An empty string at any of the four
/// steps is a value and stops the chain.
///
/// This is the **nullity** operator, not truthiness. [`build_search_path`] is
/// where the empty string disappears instead.
///
/// ```
/// use ycode::swarm::util_which::{select_path, EnvMap};
///
/// let env: EnvMap = [(String::from("PATH"), String::new())];
/// // An empty PATH is present, so the process environment is never consulted.
/// assert_eq!(select_path(&env, &[]), "");
///
/// let system: EnvMap = [(String::from("PATH"), String::from("C:\\Windows"))];
/// assert_eq!(select_path(&[], &system), "C:\\Windows");
/// ```
pub fn select_path(env: &[(String, String)], system: &[(String, String)]) -> String {
    for key in ["PATH", "Path"] {
        if let Some(value) = lookup_exact(env, key) {
            return value.to_string();
        }
    }
    for key in ["PATH", "Path"] {
        if let Some(value) = lookup_exact(system, key) {
            return value.to_string();
        }
    }
    // The `?? ""` tail of the source. The result is never absent, which is why
    // the return type is `String` and not `Option<String>`.
    String::new()
}

/// The `??` chain that produces `pathExt` in the source.
///
/// Order, exactly: `env.PATHEXT`, `env.PathExt`, `process.env.PATHEXT`,
/// `process.env.PathExt`. There is **no** default in the source, so the result
/// stays absent and the return type is `Option`. Same nullity semantics as
/// [`select_path`]: an empty string survives.
///
/// ```
/// use ycode::swarm::util_which::{select_path_ext, EnvMap};
///
/// assert_eq!(select_path_ext(&[], &[]), None);
///
/// let vide: EnvMap = [(String::from("PATHEXT"), String::new())];
/// assert_eq!(select_path_ext(&vide, &[]), Some(String::new()));
/// ```
pub fn select_path_ext(env: &[(String, String)], system: &[(String, String)]) -> Option<String> {
    for key in ["PATHEXT", "PathExt"] {
        if let Some(value) = lookup_exact(env, key) {
            return Some(value.to_string());
        }
    }
    for key in ["PATHEXT", "PathExt"] {
        if let Some(value) = lookup_exact(system, key) {
            return Some(value.to_string());
        }
    }
    // Stays `undefined`. The `which` package supplies its own default later, see
    // [`effective_path_ext`].
    None
}

/// The `?:` on `base` that produces the search path handed to `which`.
///
/// A truthy `base` gives `base + delimiter + bin`; a falsy one, and the only
/// falsy string in JavaScript is `""`, gives `bin` alone with **no** leading
/// delimiter. A string of spaces is truthy, so `" "` produces `" ;bin"`, which
/// later yields a directory named `" "`. That is faithful to the source and is
/// not trimmed here.
///
/// ```
/// use ycode::swarm::util_which::build_search_path;
///
/// assert_eq!(build_search_path("C:\\a", "C:\\bin"), "C:\\a;C:\\bin");
/// assert_eq!(build_search_path("", "C:\\bin"), "C:\\bin");
/// assert_eq!(build_search_path(" ", "C:\\bin"), " ;C:\\bin");
/// ```
pub fn build_search_path(base: &str, bin: &str) -> String {
    if base.is_empty() {
        return bin.to_string();
    }
    let mut full = String::with_capacity(base.len() + 1 + bin.len());
    full.push_str(base);
    full.push(DELIMITER);
    full.push_str(bin);
    full
}

/// Builds the exact option object the source passes to `whichPkg.sync`.
///
/// `bin` is `Global.Path.bin`, injected as data. Nothing global is read here,
/// because `Global` creates its directories on import and the port must not
/// touch the file system.
///
/// ```
/// use ycode::swarm::util_which::{build_which_call, EnvMap};
///
/// let call = build_which_call(&[], &[], "C:\\cache\\opencode\\bin");
/// assert!(call.nothrow);
/// assert_eq!(call.path, "C:\\cache\\opencode\\bin");
/// assert_eq!(call.path_ext, None);
/// ```
pub fn build_which_call(env: &[(String, String)], system: &[(String, String)], bin: &str) -> WhichCall {
    let base = select_path(env, system);
    WhichCall {
        nothrow: true,
        path: build_search_path(&base, bin),
        path_ext: select_path_ext(env, system),
    }
}

/// Splits a search path on [`DELIMITER`] and drops the empty segments.
///
/// This is `path.split(";")` followed by the emptiness filter. The split is done
/// with the character-aware `str::split`, never with a byte offset, so a segment
/// holding multi-byte characters survives whole and no slice can land inside one
/// character.
pub fn split_path_list(value: &str) -> Vec<&str> {
    value.split(DELIMITER).filter(|part| !part.is_empty()).collect()
}

/// Splits a `PATHEXT` value on [`DELIMITER`] and drops the empty segments.
///
/// Original case is kept: `PATHEXT` order is the search priority order, and the
/// comparison with a command name is case-insensitive, see
/// [`candidate_names`]. Whitespace is **not** trimmed, because a segment of
/// spaces is truthy in JavaScript and therefore kept by `which` as well.
pub fn parse_path_ext(value: &str) -> Vec<String> {
    value
        .split(DELIMITER)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// `true` when `suffix` ends `value`, ignoring ASCII case.
///
/// Character based, by design. The obvious byte-index version,
/// `value.len() - suffix.len()`, can land inside a multi-byte character and
/// panic; for `value = "cafe\u{301}"` and `suffix = "e\u{301}"` it would cut in
/// the middle of the combining sequence. This version walks the two strings in
/// reverse by character and simply stops when one runs out.
pub fn ends_with_ignore_ascii_case(value: &str, suffix: &str) -> bool {
    if suffix.is_empty() {
        return true;
    }
    let mut left = value.chars().rev();
    let mut right = suffix.chars().rev();
    loop {
        match (left.next(), right.next()) {
            (Some(a), Some(b)) => {
                if !a.eq_ignore_ascii_case(&b) {
                    return false;
                }
            }
            (_, None) => return true,  // suffix fully matched
            (None, Some(_)) => return false,  // value exhausted before suffix
        }
    }
}

/// `true` when `name` already ends with one of the `PATHEXT` extensions.
///
/// An empty extension never counts, otherwise every name would match.
pub fn has_known_extension(name: &str, path_ext: &[String]) -> bool {
    path_ext
        .iter()
        .filter(|ext| !ext.is_empty())
        .any(|ext| ends_with_ignore_ascii_case(name, ext))
}

/// Builds the file names to look for, in the order `which` tries them.
///
/// If the command already ends with a `PATHEXT` extension, it is the only
/// candidate: `which` does not append `.EXE` to a name that is already
/// executable-looking. Otherwise every extension is appended in `PATHEXT` order,
/// which is the priority order (`.COM` before `.EXE` before `.BAT` before
/// `.CMD`). An empty `PATHEXT` leaves the bare command as the single candidate.
pub fn candidate_names(cmd: &str, path_ext: &[String]) -> Vec<String> {
    if has_known_extension(cmd, path_ext) || path_ext.is_empty() {
        return vec![cmd.to_string()];
    }
    path_ext.iter().map(|ext| format!("{cmd}{ext}")).collect()
}

/// Resolves the effective extension list, given the value the source passed and
/// the process environment.
///
/// * `Some(value)`, including `Some("")`, is the caller's value verbatim. The
///   `which` package tests `!== undefined`, so an empty string is respected and
///   yields no extension at all, hence the bare command as the only candidate.
/// * `None` is `undefined`, and `which` then falls back to
///   `process.env.PATHEXT || windowsDefaultPathExt`. That `||` is a **truthiness**
///   test, so an empty process `PATHEXT` also falls through to the default. It is
///   the one place in this file where an empty string is discarded.
pub fn effective_path_ext(selected: Option<&str>, system: &[(String, String)]) -> Vec<String> {
    if let Some(value) = selected {
        return parse_path_ext(value);
    }
    let from_process = lookup_exact(system, "PATHEXT").unwrap_or("");
    if from_process.is_empty() {
        parse_path_ext(DEFAULT_PATH_EXT)
    } else {
        parse_path_ext(from_process)
    }
}

/// Joins a directory and a file name the way `path.win32.join` does.
///
/// An empty directory yields the bare name. A directory already ending with a
/// separator does not get a second one. `path.win32.normalize` is not ported in
/// full, see the module documentation.
///
/// ```
/// use ycode::swarm::util_which::join_dir;
///
/// assert_eq!(join_dir("C:\\a", "b.exe"), "C:\\a\\b.exe");
/// assert_eq!(join_dir("C:\\a\\", "b.exe"), "C:\\a\\b.exe");
/// assert_eq!(join_dir("", "b.exe"), "b.exe");
/// ```
pub fn join_dir(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        return name.to_string();
    }
    if dir.ends_with('/') || dir.ends_with(SEPARATOR) {
        let mut joined = String::with_capacity(dir.len() + name.len());
        joined.push_str(dir);
        joined.push_str(name);
        return joined;
    }
    let mut joined = String::with_capacity(dir.len() + 1 + name.len());
    joined.push_str(dir);
    joined.push(SEPARATOR);
    joined.push_str(name);
    joined
}

/// Walks directories in order, candidates in order, and returns the first hit.
///
/// This is the whole search. `probe` is the injected side effect: the caller
/// answers "is this path an existing executable file". Nothing here opens a
/// file, and `None` is the `null` the source returns, reached because `nothrow`
/// is `true`.
///
/// Order is directory-outer, extension-inner: a `.COM` in the first directory
/// beats a `.EXE` in the last one.
pub fn search_dirs<P>(dirs: &[&str], candidates: &[String], probe: &P) -> Option<String>
where
    P: Fn(&str) -> bool,
{
    for &dir in dirs {
        for candidate in candidates {
            let full = join_dir(dir, candidate);
            if probe(&full) {
                return Some(full);
            }
        }
    }
    None
}

/// Port of `which(cmd, env)` with the environment and the file system probe
/// injected.
///
/// `env` is the optional second argument of the source; `None` is `undefined` and
/// behaves exactly like an empty map, because `env?.PATH` on an absent object
/// and on an object without the key both yield `undefined`. `system` stands for
/// `process.env` and `bin` for `Global.Path.bin`. The return value mirrors the
/// final `typeof result === "string" ? result : null` of the source: a
/// [`Some`] path on a hit, [`None`] otherwise.
///
/// ```
/// use ycode::swarm::util_which::{which, EnvMap};
///
/// let env: EnvMap = [(String::from("PATH"), String::from("C:\\Windows"))];
/// let present = |path: &str| path.ends_with("node.EXE");
/// assert_eq!(
///     which("node", Some(&env), &[], "C:\\cache\\opencode\\bin", &present),
///     Some(String::from("C:\\Windows\\node.EXE"))
/// );
///
/// let absent = |_: &str| false;
/// assert_eq!(which("node", Some(&env), &[], "C:\\bin", &absent), None);
/// ```
pub fn which<P>(
    cmd: &str,
    env: Option<&[(String, String)]>,
    system: &[(String, String)],
    bin: &str,
    probe: &P,
) -> Option<String>
where
    P: Fn(&str) -> bool,
{
    let caller: &[(String, String)] = env.unwrap_or(&[]);
    let call = build_which_call(caller, system, bin);
    let extensions = effective_path_ext(call.path_ext.as_deref(), system);
    let candidates = candidate_names(cmd, &extensions);
    let dirs = split_path_list(&call.path);
    search_dirs(&dirs, &candidates, probe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeSet;

    /// A fake file system: the set of paths that exist and are executable.
    /// Nothing is read from disk, so a test is a few nanoseconds.
    fn fake<'a>(paths: &'a [&'a str]) -> impl Fn(&str) -> bool + 'a {
        let set: BTreeSet<String> = paths.iter().map(|p| p.to_string()).collect();
        move |candidate: &str| set.contains(candidate)
    }

    fn env_of(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    // --- trap 1, exact key case -------------------------------------------

    #[test]
    fn the_key_lookup_is_byte_exact_and_case_sensitive() {
        let env = env_of(&[("Path", "C:\\from-Path")]);
        // "Path" is found by "Path"...
        assert_eq!(lookup_exact(&env, "Path"), Some("C:\\from-Path"));
        // ...and NOT by "PATH", although Windows itself would not care.
        assert_eq!(lookup_exact(&env, "PATH"), None);
        assert_eq!(lookup_exact(&env, "path"), None);
    }

    #[test]
    fn the_callers_path_wins_over_the_process_one_whatever_the_case() {
        // env has "Path" only, system has "PATH" only. The chain tries
        // `env.PATH` (miss), then `env.Path` (hit).
        let env = env_of(&[("Path", "C:\\from-env")]);
        let system = env_of(&[("PATH", "C:\\from-process")]);
        assert_eq!(select_path(&env, &system), "C:\\from-env");
    }

    #[test]
    fn the_process_path_is_used_when_the_caller_sets_no_path_key() {
        let system = env_of(&[("Path", "C:\\from-process")]);
        assert_eq!(select_path(&[], &system), "C:\\from-process");
    }

    #[test]
    fn select_path_follows_the_four_step_order_then_the_empty_default() {
        // Only the last key of the chain is present: the first three steps must
        // fall through without stopping.
        let system = env_of(&[("Path", "C:\\d")]);
        assert_eq!(select_path(&[], &system), "C:\\d");

        // Nothing anywhere: the `?? ""` tail.
        assert_eq!(select_path(&[], &[]), "");

        // `env.Path` beats `process.env.PATH` even though PATH is checked first,
        // because the two sources are two separate arms of the chain.
        let env = env_of(&[("Path", "C:\\b")]);
        let system = env_of(&[("PATH", "C:\\c")]);
        assert_eq!(select_path(&env, &system), "C:\\b");
    }

    #[test]
    fn select_path_ext_uses_its_own_key_pair() {
        let env = env_of(&[("PathExt", ".EXE")]);
        let system = env_of(&[("PATHEXT", ".COM;.BAT")]);
        // `env.PATHEXT` misses, `env.PathExt` hits.
        assert_eq!(select_path_ext(&env, &system), Some(String::from(".EXE")));

        let system = env_of(&[("PathExt", ".CMD")]);
        assert_eq!(select_path_ext(&[], &system), Some(String::from(".CMD")));

        // PATHEXT never answers a PATH lookup and the other way round.
        let env = env_of(&[("PATHEXT", ".EXE")]);
        assert_eq!(select_path(&env, &[]), "");
    }

    // --- trap 2, nullity versus truthiness ---------------------------------

    #[test]
    fn an_empty_path_survives_the_coalescent() {
        // `??` yields only for null or undefined, so the empty string is a
        // value and the process environment is never read.
        let env = env_of(&[("PATH", "")]);
        let system = env_of(&[("PATH", "C:\\from-process")]);
        assert_eq!(select_path(&env, &system), "");
    }

    #[test]
    fn an_empty_pathext_survives_the_coalescent() {
        let env = env_of(&[("PATHEXT", "")]);
        let system = env_of(&[("PATHEXT", ".COM")]);
        assert_eq!(select_path_ext(&env, &system), Some(String::new()));
    }

    #[test]
    fn the_truthiness_ternary_drops_the_empty_base() {
        // The two operators disagree on the same value. `select_path` keeps the
        // empty string, `build_search_path` throws it away.
        let env = env_of(&[("PATH", "")]);
        let base = select_path(&env, &[]);
        assert_eq!(base, "");
        assert_eq!(build_search_path(&base, "C:\\bin"), "C:\\bin");
        // With a real base the two agree that it is present.
        let env = env_of(&[("PATH", "C:\\Windows")]);
        let base = select_path(&env, &[]);
        assert_eq!(build_search_path(&base, "C:\\bin"), "C:\\Windows;C:\\bin");
    }

    #[test]
    fn a_base_of_spaces_is_truthy_and_produces_a_space_directory() {
        let joined = build_search_path(" ", "C:\\bin");
        assert_eq!(joined, " ;C:\\bin");
        // The segment survives the split, so the search really visits a
        // directory named " ".
        assert_eq!(split_path_list(&joined), vec![" ", "C:\\bin"]);
    }

    #[test]
    fn the_truthiness_ternary_never_emits_a_leading_delimiter() {
        assert_eq!(build_search_path("", "C:\\bin"), "C:\\bin");
        assert!(!build_search_path("", "C:\\bin").contains(DELIMITER));
        assert_eq!(build_search_path("", ""), "");
    }

    // --- effective PATHEXT --------------------------------------------------

    #[test]
    fn an_absent_pathext_falls_back_to_the_default_list() {
        assert_eq!(
            effective_path_ext(None, &[]),
            vec![".COM", ".EXE", ".BAT", ".CMD"]
        );
    }

    #[test]
    fn an_empty_process_pathext_is_falsy_and_falls_back_to_the_default() {
        // The `||` of the `which` package tests truthiness, not nullity, so this
        // is the one spot where an empty string is discarded. It only happens on
        // the fallback arm: `Some("")` from the caller is kept.
        let system = env_of(&[("PATHEXT", "")]);
        assert_eq!(
            effective_path_ext(None, &system),
            vec![".COM", ".EXE", ".BAT", ".CMD"]
        );
        assert!(effective_path_ext(Some(""), &system).is_empty());
    }

    #[test]
    fn a_caller_pathext_replaces_the_default_entirely() {
        assert_eq!(effective_path_ext(Some(".PS1;.CMD"), &[]), vec![".PS1", ".CMD"]);
        // The process value is not consulted when the caller answered.
        let system = env_of(&[("PATHEXT", ".EXE")]);
        assert_eq!(effective_path_ext(Some(".PS1"), &system), vec![".PS1"]);
        // A non-empty process PATHEXT is used on the fallback arm.
        assert_eq!(effective_path_ext(None, &system), vec![".EXE"]);
    }

    // --- splitting ----------------------------------------------------------

    #[test]
    fn empty_segments_are_dropped_from_both_lists() {
        assert_eq!(split_path_list("A;;B;"), vec!["A", "B"]);
        assert_eq!(split_path_list(";A;B"), vec!["A", "B"]);
        assert_eq!(split_path_list(""), Vec::<&str>::new());
        assert_eq!(split_path_list(";;"), Vec::<&str>::new());
        assert_eq!(split_path_list("A"), vec!["A"]);

        assert_eq!(parse_path_ext(".COM;;.EXE;"), vec![".COM", ".EXE"]);
        assert_eq!(parse_path_ext(""), Vec::<String>::new());
        // Whitespace is not trimmed: a blank segment is truthy in JavaScript.
        assert_eq!(parse_path_ext(".EXE; "), vec![".EXE", " "]);
    }

    #[test]
    fn multi_byte_segments_survive_the_split_without_panicking() {
        // The byte-index version of this split would cut a character in half on
        // the segment boundary. `str::split` cannot.
        let path = "C:\\caf\u{e9}\\bin;C:\\Program Files\\nodejs;\\u{5c}\\u{6570}\\u{636e}";
        let segments = split_path_list(path);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0], "C:\\caf\u{e9}\\bin");
        assert_eq!(segments[1], "C:\\Program Files\\nodejs");
        assert_eq!(segments[2], "\\u{5c}\\u{6570}\\u{636e}");
        // And the segments round-trip, so nothing was lost in the split.
        assert_eq!(segments.concat(), path);
    }

    // --- candidates ---------------------------------------------------------

    #[test]
    fn every_extension_is_a_candidate_in_pathext_order() {
        let extensions = parse_path_ext(".COM;.EXE;.BAT;.CMD");
        assert_eq!(
            candidate_names("node", &extensions),
            vec!["node.COM", "node.EXE", "node.BAT", "node.CMD"]
        );
    }

    #[test]
    fn a_command_that_already_has_a_known_extension_is_not_doubled() {
        let extensions = parse_path_ext(".COM;.EXE");
        assert_eq!(candidate_names("node.exe", &extensions), vec!["node.exe"]);
        // The comparison ignores ASCII case, so `.EXE` in PATHEXT matches
        // `node.Exe` in the command.
        assert_eq!(candidate_names("node.Exe", &extensions), vec!["node.Exe"]);
        assert_eq!(candidate_names("node.COM", &extensions), vec!["node.COM"]);
    }

    #[test]
    fn an_unknown_extension_is_still_extended() {
        let extensions = parse_path_ext(".EXE");
        // `.ps1` is not a PATHEXT entry, so it is not recognised as an
        // extension and the command is suffixed anyway.
        assert_eq!(candidate_names("node.ps1", &extensions), vec!["node.ps1.EXE"]);
    }

    #[test]
    fn an_empty_extension_list_leaves_the_bare_command() {
        assert_eq!(candidate_names("node", &[]), vec!["node"]);
        // An empty PATHEXT parses to no extension at all.
        assert!(parse_path_ext("").is_empty());
        assert_eq!(candidate_names("node", &parse_path_ext("")), vec!["node"]);
    }

    #[test]
    fn the_multi_byte_suffix_match_cannot_panic_on_a_character_boundary() {
        // "café" and "é": `len() - len()` lands inside the last character of the
        // first string. The character based comparison just says no.
        assert!(!ends_with_ignore_ascii_case("caf\u{e9}", "\u{e9}"));
        assert!(ends_with_ignore_ascii_case("caf\u{e9}", "af\u{e9}"));
        assert!(ends_with_ignore_ascii_case("caf\u{e9}", "caf\u{e9}"));
        // Same trap through the extension helpers.
        let extensions = vec![String::from("\u{e9}")];
        assert!(!has_known_extension("caf\u{e9}", &extensions));
        assert_eq!(candidate_names("caf\u{e9}", &[]), vec!["caf\u{e9}"]);
    }

    #[test]
    fn suffix_matching_handles_every_length_relation() {
        assert!(ends_with_ignore_ascii_case("", ""));
        assert!(!ends_with_ignore_ascii_case("", "a"));
        assert!(!ends_with_ignore_ascii_case("a", ""));
        assert!(ends_with_ignore_ascii_case("a", "A"));
        assert!(ends_with_ignore_ascii_case("NODE.EXE", ".exe"));
        assert!(!ends_with_ignore_ascii_case("node.exe", ".exec"));
        // An empty extension never counts, otherwise everything would match.
        assert!(!has_known_extension("node", &[String::new()]));
    }

    // --- joining ------------------------------------------------------------

    #[test]
    fn joining_never_doubles_a_separator_and_never_fails_on_an_empty_dir() {
        assert_eq!(join_dir("C:\\a", "b.exe"), "C:\\a\\b.exe");
        assert_eq!(join_dir("C:\\a\\", "b.exe"), "C:\\a\\b.exe");
        assert_eq!(join_dir("C:/a/", "b.exe"), "C:/a/b.exe");
        assert_eq!(join_dir("", "b.exe"), "b.exe");
    }

    // --- the search ---------------------------------------------------------

    #[test]
    fn the_first_directory_in_path_order_wins() {
        let dirs = vec!["C:\\first", "C:\\second"];
        let candidates = candidate_names("node", &parse_path_ext(DEFAULT_PATH_EXT));
        let probe = fake(&["C:\\second\\node.EXE", "C:\\first\\node.EXE"]);
        assert_eq!(
            search_dirs(&dirs, &candidates, &probe),
            Some(String::from("C:\\first\\node.EXE"))
        );
    }

    #[test]
    fn the_extension_wins_over_the_later_directory() {
        // Directory outer, extension inner: a `.COM` in the first directory
        // beats a `.CMD` in the last one.
        let dirs = vec!["C:\\first", "C:\\last"];
        let candidates = candidate_names("node", &parse_path_ext(DEFAULT_PATH_EXT));
        let probe = fake(&["C:\\last\\node.CMD", "C:\\first\\node.COM"]);
        assert_eq!(
            search_dirs(&dirs, &candidates, &probe),
            Some(String::from("C:\\first\\node.COM"))
        );
    }

    #[test]
    fn a_miss_anywhere_is_none_which_is_the_source_null() {
        let dirs = vec!["C:\\first", "C:\\second"];
        let candidates = candidate_names("node", &parse_path_ext(DEFAULT_PATH_EXT));
        let probe = fake(&[]);
        assert_eq!(search_dirs(&dirs, &candidates, &probe), None);

        // No directory at all, for instance when the search path is only
        // delimiters. Not a panic, just a miss.
        assert_eq!(search_dirs(&[], &candidates, &probe), None);
    }

    // --- the composed call --------------------------------------------------

    #[test]
    fn the_app_bin_directory_is_searched_last() {
        // `Global.Path.bin` is appended, so a binary already on PATH wins over
        // the one opencode ships in its own cache directory.
        let env = env_of(&[("PATH", "C:\\Windows")]);
        let probe = fake(&[
            "C:\\cache\\opencode\\bin\\node.EXE",
            "C:\\Windows\\node.EXE",
        ]);
        assert_eq!(
            which("node", Some(&env), &[], "C:\\cache\\opencode\\bin", &probe),
            Some(String::from("C:\\Windows\\node.EXE"))
        );

        // And it is reached when PATH does not have it.
        let only_bin = fake(&["C:\\cache\\opencode\\bin\\node.EXE"]);
        assert_eq!(
            which("node", Some(&env), &[], "C:\\cache\\opencode\\bin", &only_bin),
            Some(String::from("C:\\cache\\opencode\\bin\\node.EXE"))
        );
    }

    #[test]
    fn the_bin_directory_is_the_whole_path_when_the_base_is_empty() {
        let env = env_of(&[("PATH", "")]);
        let probe = fake(&["C:\\bin\\node.EXE"]);
        assert_eq!(which("node", Some(&env), &[], "C:\\bin", &probe), Some(String::from("C:\\bin\\node.EXE")));
        // The bin directory is reached exactly once, not twice.
        let calls = RefCell::new(Vec::new());
        let counting = |path: &str| {
            calls.borrow_mut().push(path.to_string());
            path.ends_with("node.EXE")
        };
        which("node", Some(&env), &[], "C:\\bin", &counting);
        assert_eq!(*calls.borrow(), vec![String::from("C:\\bin\\node.COM"), String::from("C:\\bin\\node.EXE")]);
    }

    #[test]
    fn an_absent_env_behaves_exactly_like_an_empty_one() {
        // `env?.PATH` on undefined and on a keyless object both yield undefined.
        let system = env_of(&[("PATH", "C:\\Windows")]);
        let probe = fake(&["C:\\Windows\\node.EXE"]);
        let from_none = which("node", None, &system, "C:\\bin", &probe);
        let from_empty = which("node", Some(&[]), &system, "C:\\bin", &probe);
        assert_eq!(from_none, Some(String::from("C:\\Windows\\node.EXE")));
        assert_eq!(from_none, from_empty);
    }

    #[test]
    fn the_default_pathext_is_used_end_to_end_when_nothing_is_set() {
        let probe = fake(&["C:\\Windows\\node.CMD"]);
        assert_eq!(
            which("node", None, &[], "C:\\bin", &probe),
            Some(String::from("C:\\Windows\\node.CMD"))
        );
    }

    #[test]
    fn a_caller_pathext_drives_the_candidate_names_end_to_end() {
        let env = env_of(&[("PATH", "C:\\Windows"), ("PATHEXT", ".CMD;.PS1")]);
        let probe = fake(&["C:\\Windows\\node.PS1"]);
        assert_eq!(
            which("node", Some(&env), &[], "C:\\bin", &probe),
            Some(String::from("C:\\Windows\\node.PS1"))
        );

        // The same call without `node.PS1` on disk falls back to `.CMD`, the
        // first entry, and a total miss gives None.
        let missing = fake(&["C:\\Windows\\node.CMD"]);
        assert_eq!(
            which("node", Some(&env), &[], "C:\\bin", &missing),
            Some(String::from("C:\\Windows\\node.CMD"))
        );
        let none = fake(&[]);
        assert_eq!(which("node", Some(&env), &[], "C:\\bin", &none), None);
    }

    #[test]
    fn an_empty_command_never_resolves() {
        // The bare name is the only candidate, and it does not exist.
        let probe = fake(&[]);
        assert_eq!(which("", None, &[], "C:\\bin", &probe), None);
    }

    // --- the option object --------------------------------------------------

    #[test]
    fn the_call_option_keys_are_nothrow_path_and_path_ext() {
        // Trap 1 for the option object: the npm package reads `pathExt` in camel
        // case, not `path_ext` and not `PATHEXT`.
        let call = WhichCall {
            nothrow: true,
            path: String::from("C:\\a;C:\\bin"),
            path_ext: Some(String::from(".EXE")),
        };
        let json = serde_json::to_value(&call).expect("serialisation cannot fail");
        let object = json.as_object().expect("the call is a JSON object");

        let mut keys: Vec<&str> = object.keys().map(|key| key.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["nothrow", "path", "pathExt"]);
        assert_eq!(
            serde_json::to_string(&call).unwrap(),
            r#"{"nothrow":true,"path":"C:\\a;C:\\bin","pathExt":".EXE"}"#
        );
    }

    #[test]
    fn an_absent_path_ext_round_trips_as_a_missing_key() {
        // `undefined` in TypeScript is an absent key here, not a null value.
        let call = WhichCall {
            nothrow: true,
            path: String::from("C:\\bin"),
            path_ext: None,
        };
        let json = serde_json::to_string(&call).unwrap();
        assert_eq!(json, r#"{"nothrow":true,"path":"C:\\bin"}"#);

        let read: WhichCall =
            serde_json::from_str(r#"{"nothrow":true,"path":"C:\\bin"}"#).unwrap();
        assert_eq!(read, call);

        // An empty string is a different thing and must survive the round trip.
        let vide = WhichCall {
            nothrow: true,
            path: String::from("C:\\bin"),
            path_ext: Some(String::new()),
        };
        let json = serde_json::to_string(&vide).unwrap();
        assert_eq!(json, r#"{"nothrow":true,"path":"C:\\bin","pathExt":""}"#);
        assert_eq!(serde_json::from_str::<WhichCall>(&json).unwrap(), vide);
    }

    #[test]
    fn a_wrong_option_key_name_is_refused() {
        // Proof that the camel case spelling is really the one in use: a
        // snake_case key deserialises to None instead of being ignored.
        let snake: WhichCall =
            serde_json::from_str(r#"{"nothrow":true,"path":"C:\\bin","path_ext":".EXE"}"#)
                .unwrap();
        assert_eq!(snake.path_ext, None);
    }

    #[test]
    fn the_call_is_built_with_not_hrow_always_true() {
        let call = build_which_call(&[], &[], "C:\\bin");
        assert!(call.nothrow, "the source always passes nothrow true");
        assert_eq!(call.path, "C:\\bin");
        assert_eq!(call.path_ext, None);
    }
}
