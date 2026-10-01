//! The tool cluster: what the model can call, and how a call is settled.
//!
//! This directory is the whole of `opencode/packages/core/src/tool/` plus the
//! `tool-output-store` it depends on. The logic lives HERE, not in `src/core`:
//! an earlier arrangement spread it across `src/core/tool_*.rs`, which put it
//! in a different tree from the V1 `ToolBox` below and made the tool code hard
//! to find. One directory, one subject.
//!
//! One Rust module per TypeScript file, named after it:
//!
//! | Rust                     | TypeScript                                  |
//! |--------------------------|---------------------------------------------|
//! | `tool.rs`                | `tool/tool.ts` - `Tool.make`, the name gate |
//! | `registry.rs`            | `tool/registry.ts` - shadowing, permissions |
//! | `builtins.rs`            | `tool/builtins.ts` - the twelve names        |
//! | `read.rs`                | `tool/read.ts`                              |
//! | `read_filesystem.rs`     | `tool/read-filesystem.ts`                    |
//! | `write.rs`               | `tool/write.ts`                             |
//! | `edit.rs`                | `tool/edit.ts`                              |
//! | `apply_patch.rs`         | `tool/apply-patch.ts`                       |
//! | `bash.rs`                | `tool/bash.ts`                              |
//! | `glob.rs`                | `tool/glob.ts`                              |
//! | `grep.rs`                | `tool/grep.ts`                              |
//! | `question.rs`            | `tool/question.ts`                          |
//! | `skill.rs`               | `tool/skill.ts`                             |
//! | `todowrite.rs`           | `tool/todowrite.ts`                         |
//! | `webfetch.rs`            | `tool/webfetch.ts`                          |
//! | `http_body.rs`           | `tool/http-body.ts`                         |
//! | `output_store.rs`        | `tool-output-store.ts`                      |
//! | `box.rs`                 | none - the V1 `ToolBox` and its `Workspace`  |
//!
//! `box.rs` has no TypeScript counterpart in this cluster: it is an earlier
//! port of a V1-shaped tool runner. It predates the modules above, its
//! `ToolResult` return type is a plain string by design, and it is re-exported
//! here under its original names so callers outside the cluster are unaffected.
//!
//! Its module name is `toolbox`, not `box`: `box` is a reserved keyword in Rust
//! 2021 and the file is a module declaration away from being a hard error. The
//! `#[path]` attribute keeps the file named `box.rs`.
//!
//! Two conventions inside the cluster. A tool's NAME constant is the string a
//! model sends back, so a typo there does not fail a build - it fails at
//! runtime as an unknown tool; `builtins.rs` checks every name against the
//! registration gate in `tool.rs` for exactly that reason. And where a module
//! needs a shape that another module owns, it re-exports that shape rather
//! than writing a second copy: `read_filesystem` owns the entry shapes,
//! `question` re-exports the question contract from `crate::question`, and
//! `output_store` takes `Option<FileDiffInfo>` from `crate::schema::file_diff`.

pub mod apply_patch;
pub mod bash;
#[path = "box.rs"]
pub mod toolbox;
pub mod builtins;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod http_body;
pub mod output_store;
pub mod question;
pub mod read;
pub mod read_filesystem;
pub mod registry;
pub mod skill;
pub mod todowrite;
pub mod tool;
pub mod webfetch;
pub mod write;

// The V1 runner's surface, re-exported unchanged so that moving it into this
// directory is invisible from outside.
pub use toolbox::{ToolBox, ToolResult, Workspace};pub mod tools;
pub mod application_tools;
