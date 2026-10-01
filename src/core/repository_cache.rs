//! Port of the portable part of
//! `opencode/packages/core/src/repository-cache.ts`.
//!
//! Not ported: the cache itself. `ensure` clones, fetches, checks out, resets
//! and takes a file lock through Effect and the filesystem, and none of that
//! survives without the process and the disk. What is ported is the part that
//! outlives it - and that part is more than it looks.
//!
//! The eight tagged errors ARE the contract. Each carries a distinct tag string
//! beginning `RepositoryCache`, and a caller distinguishing "the clone failed"
//! from "the branch is invalid" matches on that tag rather than on a message.
//! Four of them carry fields the others do not, which is the detail a flat
//! struct gets wrong: the checkout error carries BOTH a repository and a
//! branch, the lock error carries a local path and no repository, and the
//! operation error carries an operation and a path and neither. So the payload
//! is modelled per variant rather than merged.
//!
//! `parseRemote` and `validateBranch` are the two functions here that do real
//! work, and neither does: both delegate to `core::repository` and re-wrap the
//! failure in their own error type, keeping the original message. That is the
//! whole reason they exist as separate functions - they translate the
//! repository module's refusals into the cache's vocabulary. They are ported by
//! delegating, not by reimplementing: `core::repository` already holds
//! `parse_remote` and `validate_branch`, and a second copy of either would be a
//! second place for the rules to drift.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::repository::{self, Reference};

/// `Result.status`: what `ensure` did to get the cache into shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Already present and nothing to do.
    Cached,
    /// Cloned for the first time.
    Cloned,
    /// Already present, brought up to date.
    Refreshed,
}

impl Status {
    /// The literal on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Cached => "cached",
            Status::Cloned => "cloned",
            Status::Refreshed => "refreshed",
        }
    }
}

/// `Result`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Result_ {
    pub repository: String,
    pub host: String,
    pub remote: String,
    #[serde(rename = "localPath")]
    pub local_path: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// `EnsureInput`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsureInput {
    pub reference: Reference,
    pub refresh: Option<bool>,
    pub branch: Option<String>,
}

/// The eight error variants, each with its own payload.
///
/// Named after the TS classes rather than the tags, which are longer and would
/// make the enum unreadable; `tag` gives the wire name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag")]
pub enum CacheError {
    #[serde(rename = "RepositoryCacheInvalidRepositoryError")]
    InvalidRepository { repository: String, message: String },
    #[serde(rename = "RepositoryCacheInvalidBranchError")]
    InvalidBranch { branch: String, message: String },
    #[serde(rename = "RepositoryCacheCloneFailedError")]
    CloneFailed { repository: String, message: String },
    #[serde(rename = "RepositoryCacheFetchFailedError")]
    FetchFailed { repository: String, message: String },
    /// The only one carrying a branch as well as a repository.
    #[serde(rename = "RepositoryCacheCheckoutFailedError")]
    CheckoutFailed { repository: String, branch: String, message: String },
    #[serde(rename = "RepositoryCacheResetFailedError")]
    ResetFailed { repository: String, message: String },
    /// The only one carrying a local path and no repository.
    #[serde(rename = "RepositoryCacheLockFailedError")]
    LockFailed { local_path: String, message: String },
    /// The only one carrying an operation and a path.
    #[serde(rename = "RepositoryCacheOperationError")]
    Operation { operation: String, path: String, message: String },
}

impl CacheError {
    /// `isError`: every inhabitant of the union is a `RepositoryCache.Error`,
    /// so the type guard is total.
    pub fn is_error(&self) -> bool {
        true
    }

    /// The `message` field, which every variant carries.
    pub fn message(&self) -> &str {
        match self {
            CacheError::InvalidRepository { message, .. }
            | CacheError::InvalidBranch { message, .. }
            | CacheError::CloneFailed { message, .. }
            | CacheError::FetchFailed { message, .. }
            | CacheError::CheckoutFailed { message, .. }
            | CacheError::ResetFailed { message, .. }
            | CacheError::LockFailed { message, .. }
            | CacheError::Operation { message, .. } => message,
        }
    }
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for CacheError {}

/// `parseRemote`: the repository module's refusal, re-wrapped in the cache's
/// own error type, message preserved.
pub fn parse_remote(repository_input: &str) -> Result<Reference, CacheError> {
    repository::parse_remote(repository_input).map_err(|error| CacheError::InvalidRepository {
        repository: repository_input.to_string(),
        message: error.to_string(),
    })
}

/// `validateBranch`: likewise, re-wrapped as a branch error.
pub fn validate_branch(branch: &str) -> Result<(), CacheError> {
    repository::validate_branch(branch).map_err(|error| CacheError::InvalidBranch {
        branch: branch.to_string(),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_statuses_are_the_wire_literals() {
        assert_eq!(serde_json::to_value(Status::Cached).unwrap(), serde_json::json!("cached"));
        assert_eq!(serde_json::to_value(Status::Cloned).unwrap(), serde_json::json!("cloned"));
        assert_eq!(serde_json::to_value(Status::Refreshed).unwrap(), serde_json::json!("refreshed"));
        assert_eq!(Status::Refreshed.as_str(), "refreshed");
    }

    #[test]
    fn a_result_omits_an_absent_head_and_branch() {
        let r = Result_ {
            repository: "o/r".into(),
            host: "github.com".into(),
            remote: "https://github.com/o/r.git".into(),
            local_path: "/cache/r".into(),
            status: Status::Cached,
            head: None,
            branch: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("head").is_none());
        assert!(v.get("branch").is_none());
        assert_eq!(v["localPath"], serde_json::json!("/cache/r"));
    }

    #[test]
    fn a_result_writes_the_fields_it_has() {
        let r = Result_ {
            repository: "o/r".into(),
            host: "github.com".into(),
            remote: "https://github.com/o/r.git".into(),
            local_path: "/cache/r".into(),
            status: Status::Refreshed,
            head: Some("abc".into()),
            branch: Some("main".into()),
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["head"], serde_json::json!("abc"));
        assert_eq!(v["branch"], serde_json::json!("main"));
        assert_eq!(v["status"], serde_json::json!("refreshed"));
    }

    #[test]
    fn every_error_has_a_distinct_tag() {
        let errors = vec![
            CacheError::InvalidRepository { repository: "r".into(), message: "m".into() },
            CacheError::InvalidBranch { branch: "b".into(), message: "m".into() },
            CacheError::CloneFailed { repository: "r".into(), message: "m".into() },
            CacheError::FetchFailed { repository: "r".into(), message: "m".into() },
            CacheError::CheckoutFailed {
                repository: "r".into(),
                branch: "b".into(),
                message: "m".into(),
            },
            CacheError::ResetFailed { repository: "r".into(), message: "m".into() },
            CacheError::LockFailed { local_path: "/p".into(), message: "m".into() },
            CacheError::Operation {
                operation: "fetch".into(),
                path: "/p".into(),
                message: "m".into(),
            },
        ];
        let tags: Vec<String> = errors
            .iter()
            .map(|error| {
                serde_json::to_value(error).unwrap()["_tag"].as_str().unwrap().to_string()
            })
            .collect();
        assert_eq!(tags.len(), 8);
        let mut unique = tags.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 8, "no two errors share a tag: {:?}", tags);
        assert!(tags.iter().all(|tag| tag.starts_with("RepositoryCache")));
    }

    #[test]
    fn the_checkout_error_carries_a_branch_and_the_lock_one_a_path() {
        let checkout = CacheError::CheckoutFailed {
            repository: "r".into(),
            branch: "main".into(),
            message: "m".into(),
        };
        let v = serde_json::to_value(&checkout).unwrap();
        assert_eq!(v["branch"], serde_json::json!("main"));
        assert_eq!(v["repository"], serde_json::json!("r"));

        let lock = CacheError::LockFailed { local_path: "/p".into(), message: "m".into() };
        let v = serde_json::to_value(&lock).unwrap();
        assert_eq!(v["localPath"], serde_json::json!("/p"));
        assert!(v.get("repository").is_none(), "the lock error has no repository");
    }

    #[test]
    fn the_operation_error_carries_an_operation_and_a_path() {
        let e = CacheError::Operation {
            operation: "fetch".into(),
            path: "/p".into(),
            message: "m".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["operation"], serde_json::json!("fetch"));
        assert_eq!(v["path"], serde_json::json!("/p"));
    }

    #[test]
    fn parse_remote_rewraps_a_bad_reference_as_a_repository_error() {
        let err = parse_remote("nonsense").unwrap_err();
        match &err {
            CacheError::InvalidRepository { repository, message } => {
                assert_eq!(repository, "nonsense");
                assert_eq!(message, err.message());
                assert!(!message.is_empty(), "the original message is carried over");
            }
            other => panic!("expected a repository error, got {:?}", other),
        }
    }

    #[test]
    fn parse_remote_rewraps_a_local_reference_as_a_repository_error_too() {
        // `file:` is a valid reference but not a remote one; the cache refuses
        // it and the refusal arrives as the cache's own error type.
        let err = parse_remote("file:///a/b").unwrap_err();
        assert!(matches!(err, CacheError::InvalidRepository { .. }));
    }

    #[test]
    fn parse_remote_accepts_the_usual_forms() {
        assert!(parse_remote("owner/repo").is_ok());
        assert!(parse_remote("git@github.com:o/r.git").is_ok());
    }

    #[test]
    fn validate_branch_rewraps_the_refusal_and_names_the_branch() {
        let err = validate_branch("-force").unwrap_err();
        match &err {
            CacheError::InvalidBranch { branch, message } => {
                assert_eq!(branch, "-force");
                assert!(message.contains("cannot start with -"));
            }
            other => panic!("expected a branch error, got {:?}", other),
        }
    }

    #[test]
    fn validate_branch_accepts_the_usual_branches() {
        assert!(validate_branch("main").is_ok());
        assert!(validate_branch("feature/x").is_ok());
    }

    #[test]
    fn the_two_wrappers_keep_the_repository_module_rules() {
        // The point of delegating: a branch the repository module accepts is
        // accepted here, and vice versa. If this ever fails, one of the two
        // copies of the rule has drifted.
        for branch in ["main", "release-1.2", "-bad", "a..b", "a b", ""] {
            assert_eq!(
                validate_branch(branch).is_ok(),
                repository::validate_branch(branch).is_ok(),
                "{}",
                branch
            );
        }
    }

    #[test]
    fn every_error_is_an_error_and_shows_its_message() {
        let e = CacheError::FetchFailed { repository: "r".into(), message: "network down".into() };
        assert!(e.is_error());
        assert_eq!(e.to_string(), "network down");
    }

    #[test]
    fn an_error_round_trips_through_its_tag() {
        let e = CacheError::ResetFailed { repository: "r".into(), message: "m".into() };
        let v = serde_json::to_value(&e).unwrap();
        let back: CacheError = serde_json::from_value(v).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn an_unknown_tag_is_refused_on_the_way_back() {
        let raw = serde_json::json!({
            "_tag": "RepositoryCacheNopeError",
            "repository": "r",
            "message": "m",
        });
        assert!(serde_json::from_value::<CacheError>(raw).is_err());
    }
}