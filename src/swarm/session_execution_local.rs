//! Rust port of `packages/core/src/session/execution/local.ts` (46 lines).
//!
//! That file is the **current-process** implementation of the Session execution
//! service. It has no algorithm of its own. It wires three collaborators
//! together and delegates every decision to one of them:
//!
//! ```text
//! SessionExecution.Service        what callers see
//!   |-- active    -> coordinator.active
//!   |-- interrupt -> coordinator.interrupt
//!   |-- resume    -> coordinator.run      <-- the only name that changes
//!   `-- wake      -> coordinator.wake
//!
//! coordinator (SessionRunCoordinator.make)
//!   `-- drain(sessionID, force)        the only code with a body
//!         1. store.get(sessionID)      -> Effect.die when it is undefined
//!         2. locations.get(location)   -> the Layer that owns the runner
//!         3. runner.run({sessionID, force})
//!         4. tapCause: log unless the cause holds interrupts only
//! ```
//!
//! So this port has exactly two jobs: the **routing** of the four service
//! members, and the **body of `drain`**. Everything else in the source belongs
//! to a module owned by somebody else and is named here, not reimplemented.
//!
//! ## What is ported
//!
//! - [`Operation`] and its two name tables: the member exposed by
//!   `SessionExecution.Interface` and the coordinator member it is bound to.
//! - [`action_for`]: the guard clauses those four members land on inside
//!   `run-coordinator.ts` (`run` joining / starting, `wake` coalescing,
//!   `interrupt` no-oping, `active` snapshotting), plus the `force` flag each
//!   `start` receives. It is a pure function from `(operation, entry state)` to
//!   one decision.
//! - [`drain`]: the store lookup, the `Effect.die` on a missing session, the
//!   location lookup, the exact `RunInput` handed to the runner, and the
//!   two-channel outcome (typed `RunError` versus defect).
//! - [`log_policy`]: the `Effect.tapCause` decision, including the
//!   `Cause.hasInterruptsOnly` test and the `{ sessionID }` log annotation.
//! - [`node`]: the `makeGlobalNode` descriptor, its kind, its service key, its
//!   tag, and the two dependencies in the order the source lists them.
//!
//! ## What is NOT ported, and why
//!
//! - **`SessionRunCoordinator` itself.** `make` builds a `Map<Key, Entry>`,
//!   a `FiberSet`, one `Deferred` per entry, and a `Scope`. `run` blocks on
//!   `Deferred.await`, `interrupt` calls `Fiber.interrupt`, and the whole
//!   `run` body sits inside `Effect.uninterruptibleMask`. None of that can be
//!   written here without a real lock, a real thread or a real async runtime,
//!   and this port writes none of those: **no `Mutex`, no `RwLock`, no
//!   `Condvar`, no `thread`, no `spawn`, no `Arc`, no `async`, no `sleep`, no
//!   blocking wait, and no loop whose exit condition is not checked on the
//!   spot.** Every function below returns; none can hang, so no test in this
//!   file can hang either.
//!   The logic is therefore ported as **pure decisions** ([`action_for`],
//!   [`drain`], [`log_policy`]) and the *scheduling* is left to whoever owns
//!   the runtime. This module deliberately does **not** depend on
//!   `crate::core::session::run_coordinator`, which is the owner of that
//!   concurrency and is currently the subject of a hanging test: depending on
//!   it would drag `Mutex` and `Condvar` into this file's dependency graph for
//!   no gain, since the only thing this file needs from it is the name of a
//!   member and the branch it takes.
//! - **The Effect layer graph.** `Layer.effect`, `SessionStore.Service`,
//!   `LocationServiceMap.Service`, `SessionRunner.Service.use` and
//!   `Effect.provide` all read their value from an ambient environment. There
//!   is no environment in Rust, so the three collaborators are **arguments**:
//!   [`SessionStore`] and [`LocationServices`], both narrowed to the single
//!   member this file actually calls (`get`). The dependency graph is the same
//!   one, written explicitly instead of implicitly.
//! - **`Effect.die` is not an error value.** It is a *defect*: it leaves the
//!   typed channel `E = SessionRunner.RunError` untouched. [`Drain`] keeps the
//!   two apart (`Ran { result }` versus `SessionNotFound`), and
//!   [`Drain::cause`] reports the defect on the same cause list the logging
//!   rule inspects, because in the source `tapCause` sees both.
//! - **`Effect.logError` and `annotateLogs` themselves.** What is ported is the
//!   *decision* (log or stay silent, with which message and which annotation,
//!   [`LogPolicy`]) and not the logging runtime.
//! - **`Effect.fnUntraced`.** The drain is deliberately untraced, so the
//!   source attaches no span and no name to it. Nothing is ported, and in
//!   particular no span name is invented: adding one would be a silent
//!   behavioural difference in the traces.
//! - **`export * as SessionExecutionLocal from "./local"`** (last line) is a
//!   self re-export of the module namespace onto itself, like the other
//!   `export * as X from "./x"` lines in this repository. It carries no runtime
//!   logic, so there is nothing to translate.
//!
//! ## A typing question this file cannot settle on its own
//!
//! The drain is declared as returning `Effect<void, SessionRunner.RunError>`
//! (line 16 fixes `E = SessionRunner.RunError`), yet line 21 wraps the runner
//! call in `Effect.provide(locations.get(session.location))`, and
//! `LocationServiceMap.get` is a `LayerMap` lookup that can itself fail with
//! `LocationError`. Whether that widens the drain's error channel, or whether
//! the layer's build failure is erased by `Layer.unwrap`, cannot be decided
//! from this file: it needs the real signature of `effect`'s `LayerMap`, and
//! the `effect` package is not installed on this machine. The port therefore
//! keeps the two failures **separate**, [`Drain::LocationUnavailable`] beside
//! [`Drain::Ran`], and never folds a location failure into
//! [`RunError`]. If the reviewer has the library at hand, this is the first
//! thing to check.
//!
//! ## Traps of the source that this port must not get wrong
//!
//! - **`sessionID` is upper case.** It is the drain parameter
//!   (`function* (sessionID, force)`), the key of the log annotation
//!   (`Effect.annotateLogs({ sessionID })`), the field of the object handed to
//!   the runner (`{ sessionID, force }`), and the placeholder in the defect
//!   message. Writing `session_id` in the JSON would compile, pass every logic
//!   test, and break only at the exchange with the TypeScript. Hence the
//!   explicit `#[serde(rename = "sessionID")]` on [`LogAnnotation::session_id`],
//!   and one test that serialises the value and one that refuses the
//!   snake_case form on read.
//! - **`if (!session)` is a TRUTHINESS test, not a nullity test.** The value
//!   it guards is a `Session.Info` object or `undefined`, and an object is
//!   always truthy, so the two readings coincide *here only*. The consequence
//!   is that a session whose every string field is empty is still a found
//!   session: there is no "empty means absent" rule anywhere near it.
//! - **The ternary `hasInterruptsOnly(cause) ? Effect.void : logError(...)` is
//!   truthiness on a `boolean`.** There is no `??` in this file, so no
//!   nullity/truthiness gap is hidden in a coalescent. `false` takes the log
//!   branch, and a cause that mixes one interrupt with one failure is *not*
//!   interrupts-only, so it is logged.
//! - **`force` is a plain positional `boolean` with no default.** `resume`
//!   reaches `start(key, entry, true)` and a `wake` on an idle key reaches
//!   `start(key, next, false)`. The two are different code paths, so
//!   [`Action::force`] is tested separately from the choice of the action.
//! - **`entry?.owner === undefined`** inside `interrupt` is a *nullity* test
//!   reached through this file's `interrupt` member: an entry that exists but
//!   has no owner is a no-op, exactly like a key that has no entry at all.

use serde::{Deserialize, Serialize};

use crate::core::session::execution::SERVICE_TAG as EXECUTION_SERVICE_TAG;
use crate::core::session::schema::{Info as SessionInfo, LocationRef, SessionId};
use crate::swarm::location_service_map::SERVICE_KEY as LOCATION_SERVICE_MAP_KEY;
pub use crate::swarm::session_runner_index::Interface as SessionRunner;
use crate::swarm::session_runner_index::{RunError, RunInput};

/// Tag of the service this file implements.
///
/// Copied from `crate::core::session::execution::SERVICE_TAG`, which is
/// itself the verbatim key declared by
/// `export class Service extends Context.Service<...>()("@opencode/v2/SessionExecution")`
/// in `session/execution.ts:21`. It is **imported, not redeclared**: two
/// definitions of the same context key would diverge in silence.
pub const SERVICE_TAG: &str = EXECUTION_SERVICE_TAG;

/// Key of the first dependency, from `export class Service ... ("@opencode/v2/SessionStore")`
/// in `session/store.ts:26`.
///
/// The store module is not part of this port, so the string is the only thing
/// that can be carried: `node` needs the dependency's *name*, not its
/// implementation.
pub const SESSION_STORE_TAG: &str = "@opencode/v2/SessionStore";

/// Tag of the node, from `tags.make("global")` in `effect/app-node.ts:11`.
///
/// `makeGlobalNode` is `tags.make("global")`, and `tags` is
/// `LayerNode.tags({ location: ["global"], global: [] })`, so the tag attached
/// to the node is the **string** `"global"` (`values.global = makeTag("global")`).
/// The `[]` in the configuration is the list of tags a *dependency* of a global
/// node may carry, not the tag of the node itself.
pub const NODE_TAG: &str = "global";

/// The four members of `SessionExecution.Interface`, in the order the source
/// lists them in `Service.of({...})`.
pub const OPERATIONS: [Operation; 4] = [
    Operation::Active,
    Operation::Interrupt,
    Operation::Resume,
    Operation::Wake,
];

/// Message of `Effect.logError("Failed to drain Session", cause)`.
pub const DRAIN_LOG_MESSAGE: &str = "Failed to drain Session";

/// Prefix of the defect message, from `Effect.die(\`Session not found: ${sessionID}\`)`.
pub const SESSION_NOT_FOUND_PREFIX: &str = "Session not found: ";

/// One member of the service, and the coordinator member it delegates to.
///
/// This is the port of the four lines of `SessionExecution.Service.of({...})`:
///
/// ```ts
/// active: coordinator.active,
/// interrupt: coordinator.interrupt,
/// resume: coordinator.run,
/// wake: coordinator.wake,
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Operation {
    /// `active: coordinator.active`.
    Active,
    /// `interrupt: coordinator.interrupt`.
    Interrupt,
    /// `resume: coordinator.run`.
    Resume,
    /// `wake: coordinator.wake`.
    Wake,
}

impl Operation {
    /// The name the service publishes, i.e. the key of
    /// `SessionExecution.Interface`.
    pub fn service_member(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Interrupt => "interrupt",
            Self::Resume => "resume",
            Self::Wake => "wake",
        }
    }

    /// The name of the coordinator member it is bound to.
    ///
    /// `resume` is the single rename in the whole file: the service calls it
    /// `resume`, the coordinator calls it `run`.
    pub fn coordinator_member(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Interrupt => "interrupt",
            Self::Resume => "run",
            Self::Wake => "wake",
        }
    }

    /// The key the service itself carries, `@opencode/v2/SessionExecution`.
    pub fn service_tag(self) -> &'static str {
        SERVICE_TAG
    }
}

/// The wiring of [`drain`] into the four service members, in source order.
///
/// Port of `SessionRunCoordinator.make<SessionSchema.ID, SessionRunner.RunError>({ drain })`
/// followed by `Service.of({...})`: the two type arguments of the coordinator
/// are `SessionSchema.ID` (so the key is the session id, already carried by
/// [`drain`]'s `session_id` parameter) and `SessionRunner.RunError` (so
/// [`RunError`] is the typed error channel of the whole service).
pub fn wiring() -> Vec<(Operation, &'static str, &'static str)> {
    OPERATIONS
        .iter()
        .map(|op| (*op, op.service_member(), op.coordinator_member()))
        .collect()
}

/// The visible part of one `Entry<E>` of `run-coordinator.ts`, for the single
/// key an operation is about.
///
/// The entry also holds a `Deferred` and a fiber; both are runtime objects with
/// no value to inspect synchronously, and both are covered by [`KeyState::owned`]
/// plus by the fact that this port never blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyState {
    /// `entry.owner !== undefined`: a fiber is currently running the drain.
    pub owned: bool,
    /// `entry.stopping`: an interruption was requested.
    pub stopping: bool,
    /// `entry.pendingWake`: a follow-up was registered during the execution.
    pub pending_wake: bool,
}

impl KeyState {
    /// An entry that is running, with no interruption and no pending follow-up.
    /// This is the state `makeEntry()` produces right after a `start`.
    pub const fn running() -> Self {
        Self {
            owned: true,
            stopping: false,
            pending_wake: false,
        }
    }
}

/// What one service member does to one key, decided from the entry state alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `active`: `Effect.sync(() => new Set(active.keys()))`.
    SnapshotActive,
    /// `run` on a live entry: `restore(Deferred.await(entry.done))`, that is,
    /// join the execution already in progress.
    JoinActive,
    /// `run` on an idle key: `makeEntry()`, `active.set`, `start(key, entry, true)`.
    StartForced,
    /// `run` on a stopping entry: await the entry, then run again.
    AwaitThenRetry,
    /// `wake` on a live entry: `entry.pendingWake = true`, and return.
    MarkPendingWake,
    /// `wake` on an idle key: `makeEntry()`, `active.set`, `start(key, next, false)`.
    StartUnforced,
    /// `interrupt` with no entry, or with an entry that has no owner:
    /// `return Effect.void`.
    Noop,
    /// `interrupt` on an owned entry: `entry.stopping = true`,
    /// `entry.pendingWake = false`, `Fiber.interrupt(entry.owner)`.
    InterruptOwner,
}

impl Action {
    /// The `force` argument `start` receives, or `None` when the action does
    /// not start anything.
    ///
    /// `true` comes from `resume` (an explicit run), `false` from every
    /// follow-up a `wake` produces.
    pub fn force(self) -> Option<bool> {
        match self {
            Self::StartForced => Some(true),
            Self::StartUnforced | Self::MarkPendingWake => Some(false),
            _ => None,
        }
    }
}

/// The pure decision behind the four delegations.
///
/// `state` is `None` when `active.get(key)` returned `undefined`. Every branch
/// is decided from that pair alone, which is what makes the port testable
/// without a fiber.
pub fn action_for(operation: Operation, state: Option<KeyState>) -> Action {
    match operation {
        Operation::Active => Action::SnapshotActive,
        Operation::Resume => match state {
            None => Action::StartForced,
            Some(entry) if entry.stopping => Action::AwaitThenRetry,
            Some(_) => Action::JoinActive,
        },
        Operation::Wake => match state {
            None => Action::StartUnforced,
            Some(_) => Action::MarkPendingWake,
        },
        Operation::Interrupt => match state {
            Some(entry) if entry.owned => Action::InterruptOwner,
            _ => Action::Noop,
        },
    }
}

/// The only member of `SessionStore.Interface` this file calls.
///
/// `get` returns `Effect<SessionSchema.Info | undefined>` with **no** error
/// channel, which is why [`SessionStore::get`] returns an `Option` and not a
/// `Result`. An `undefined` row is an ordinary outcome, not a failure.
pub trait SessionStore {
    /// `store.get(sessionID)`.
    fn get(&self, session_id: &SessionId) -> Option<SessionInfo>;
}

/// The per-Location services, narrowed to the one member this file calls.
///
/// In the source, `locations.get(session.location)` yields the whole service
/// layer of a working directory, and `SessionRunner.Service.use` then picks the
/// runner out of it. The two types of that layer are not fixed here: the layer
/// is assembled by `location-services.ts` from thirty-six nodes, none of which
/// is ported, so they are associated types. The error type is the
/// `LocationError` of that layer, kept apart from [`RunError`] on purpose (see
/// the module documentation).
pub trait LocationServices {
    /// The runner owned by this Location, i.e. the value `SessionRunner.Service`
    /// resolves to once the Location layer is provided.
    type Runner: SessionRunner;

    /// The Location layer's own error channel.
    type Error;

    /// `locations.get(locationRef)`.
    fn get(&self, location: &LocationRef) -> Result<Self::Runner, Self::Error>;
}

/// The result of one `drain` call.
///
/// The source builds an `Effect`, which is a *description* of work; running it
/// is the runtime's job. This enum is the description, already evaluated
/// against the two injected ports. It keeps the three ways out of the drain
/// apart, because the source does:
///
/// ```ts
/// if (!session) return yield* Effect.die(`Session not found: ${sessionID}`)  // defect
/// ... .pipe(Effect.provide(locations.get(session.location)))                   // LocationError
/// ... runner.run({ sessionID, force })                                         // RunError
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Drain<E> {
    /// `store.get` returned `undefined`: the drain dies instead of failing.
    SessionNotFound(SessionId),
    /// The Location layer could not be built, so the runner was never reached.
    LocationUnavailable {
        /// The session the drain was about. The session **was** found here; only
        /// its Location could not be built.
        session_id: SessionId,
        /// The Location the source would have provided.
        location: LocationRef,
        /// The failure the layer reported.
        error: E,
    },
    /// The runner ran, or refused to run.
    Ran {
        /// The Location whose runner was used.
        location: LocationRef,
        /// The exact object handed to `runner.run`, `{ sessionID, force }`.
        input: RunInput,
        /// The typed failure channel of the drain.
        result: Result<(), RunError>,
    },
}

impl<E> Drain<E> {
    /// The `force` flag the drain forwarded to the runner.
    ///
    /// `None` when the runner was never reached.
    pub fn force(&self) -> Option<bool> {
        match self {
            Self::Ran { input, .. } => Some(input.force),
            _ => None,
        }
    }

    /// The session the drain was about, always, including when the session does
    /// not exist. `""` is a legitimate key here: nothing in the source filters
    /// it, and `store.get("")` simply returns whatever the store returns.
    pub fn session_id(&self) -> SessionId {
        match self {
            Self::SessionNotFound(session_id) => session_id.clone(),
            Self::Ran { input, .. } => input.session_id.clone(),
            Self::LocationUnavailable { session_id, .. } => session_id.clone(),
        }
    }

    /// The message of the `Effect.die`, or `None` when the drain did not die.
    ///
    /// ```text
    /// `Session not found: ${sessionID}`
    /// ```
    pub fn defect_message(&self) -> Option<String> {
        match self {
            Self::SessionNotFound(session_id) => {
                Some(format!("{}{}", SESSION_NOT_FOUND_PREFIX, session_id))
            }
            _ => None,
        }
    }

    /// What `tapCause` would see, or `None` when the drain succeeded.
    ///
    /// A successful `runner.run` has no cause at all, so nothing is logged. A
    /// `die` and a `RunError` both land on the cause list, which is why they
    /// are both reported here: the logging rule cannot tell them apart without
    /// looking at the cause type.
    pub fn cause(&self) -> Option<Cause> {
        match self {
            Self::SessionNotFound(session_id) => Some(Cause::Defect(format!(
                "{}{}",
                SESSION_NOT_FOUND_PREFIX, session_id
            ))),
            Self::LocationUnavailable { location, .. } => {
                Some(Cause::LocationUnavailable(location.clone()))
            }
            Self::Ran { result, .. } => result.as_ref().err().map(|e| Cause::Failure(*e)),
        }
    }

    /// The typed channel of the drain, with the defect reported separately.
    ///
    /// `Ok(Ok(()))` is a success, `Ok(Err(e))` a `RunError`, `Err(defect)` a
    /// `die`. The nesting is the point: in the source a `die` never appears in
    /// `E`, so flattening the two would invent a seventh `RunError` member.
    pub fn typed_result(&self) -> Result<Result<(), RunError>, Defect> {
        match self {
            Self::SessionNotFound(session_id) => Err(Defect::SessionNotFound(format!(
                "{}{}",
                SESSION_NOT_FOUND_PREFIX, session_id
            ))),
            Self::LocationUnavailable { .. } => Err(Defect::LocationUnavailable),
            Self::Ran { result, .. } => Ok(result.clone()),
        }
    }
}

/// What a `die` carries: not an error value but a reason.
///
/// [`Defect::SessionNotFound`] is the only defect the source writes down, as a
/// string. [`Defect::LocationUnavailable`] is this port's own marker for the
/// one exit the source leaves untyped, and it carries **no** invented message:
/// the source writes none there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Defect {
    /// ``Effect.die(`Session not found: ${sessionID}`)``.
    SessionNotFound(String),
    /// The provided Location layer could not be built.
    LocationUnavailable,
}

impl std::fmt::Display for Defect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionNotFound(message) => f.write_str(message),
            Self::LocationUnavailable => f.write_str("Location layer unavailable"),
        }
    }
}

impl std::error::Error for Defect {}

/// The port of the drain body.
///
/// ```ts
/// drain: Effect.fnUntraced(function* (sessionID, force) {
///   const session = yield* store.get(sessionID)
///   if (!session) return yield* Effect.die(`Session not found: ${sessionID}`)
///   return yield* SessionRunner.Service.use((runner) => runner.run({ sessionID, force })).pipe(
///     Effect.provide(locations.get(session.location)),
///     Effect.tapCause(...),
///   )
/// })
/// ```
///
/// Every step is a plain call that returns: the store lookup, the Location
/// lookup and the runner call. Nothing here can block, so nothing here can hang.
pub fn drain<S, L>(store: &S, locations: &L, session_id: &SessionId, force: bool) -> Drain<L::Error>
where
    S: SessionStore,
    L: LocationServices,
{
    let session = match store.get(session_id) {
        Some(session) => session,
        None => return Drain::SessionNotFound(session_id.clone()),
    };
    let location = session.location.clone();
    let runner = match locations.get(&location) {
        Ok(runner) => runner,
        Err(error) => {
            return Drain::LocationUnavailable {
                session_id: session_id.clone(),
                location,
                error,
            };
        }
    };
    let input = RunInput::new(session_id.clone(), force);
    let result = runner.run(input.clone());
    Drain::Ran {
        location,
        input,
        result,
    }
}

/// One element of the `Cause` that `Effect.tapCause` receives.
///
/// The source inspects a whole `Cause`, which is a set. A drain contributes at
/// most one element, but [`log_policy`] takes a slice so the "interrupts only"
/// rule keeps its set semantics, and so a mixed cause can be tested.
///
/// `Eq` is not derived: [`LocationRef`] is **imported**, and it derives only
/// `Debug, Clone, PartialEq, Serialize, Deserialize`. Adding `Eq` here would
/// mean either failing to compile or shadowing the imported type with a local
/// copy, and a local copy is exactly the silent divergence this portage is
/// meant to avoid.
#[derive(Debug, Clone, PartialEq)]
pub enum Cause {
    /// An interruption, from `SessionExecution.interrupt` reaching
    /// `Fiber.interrupt`.
    Interrupt,
    /// A `RunError` from the runner.
    Failure(RunError),
    /// A `die`, which in the source carries a string.
    Defect(String),
    /// The provided Location layer could not be built.
    LocationUnavailable(LocationRef),
}

impl Cause {
    /// Whether this single cause is an interruption.
    pub fn is_interrupt(&self) -> bool {
        matches!(self, Self::Interrupt)
    }
}

/// `Cause.hasInterruptsOnly(cause)`.
///
/// True when the cause holds **at least one** element and every element is an
/// interruption. An empty cause is not interrupts-only: `tapCause` only ever
/// runs on a real exit, so an empty list is unreachable in the source, and
/// answering "log" is the direction that does not swallow an error.
pub fn has_interrupts_only(causes: &[Cause]) -> bool {
    !causes.is_empty() && causes.iter().all(Cause::is_interrupt)
}

/// The annotation added to the log line, `Effect.annotateLogs({ sessionID })`.
///
/// The key is `sessionID` with the upper case, because that is what the source
/// writes and what a log consumer reads back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogAnnotation {
    /// The session being drained.
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
}

/// What `Effect.tapCause` decides to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogPolicy {
    /// `Effect.void`: the cause holds interrupts only, and the interruption is
    /// not an error worth reporting.
    Silent,
    /// `Effect.logError("Failed to drain Session", cause).pipe(Effect.annotateLogs({ sessionID }))`.
    Error {
        /// Always [`DRAIN_LOG_MESSAGE`].
        message: String,
        /// The session the annotation names.
        annotation: LogAnnotation,
    },
}

impl LogPolicy {
    /// The message that would be logged, or `None` for [`LogPolicy::Silent`].
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Silent => None,
            Self::Error { message, .. } => Some(message.as_str()),
        }
    }
}

/// The `tapCause` decision, as a pure function.
///
/// The ternary of the source is a **truthiness** test on the `boolean` returned
/// by `hasInterruptsOnly`, so `false` is what reaches the log branch.
pub fn log_policy(causes: &[Cause], session_id: &SessionId) -> LogPolicy {
    if has_interrupts_only(causes) {
        return LogPolicy::Silent;
    }
    LogPolicy::Error {
        message: String::from(DRAIN_LOG_MESSAGE),
        annotation: LogAnnotation {
            session_id: session_id.clone(),
        },
    }
}

/// The kind of a `LayerNode`.
///
/// Only the value this file produces is ported: `makeGlobalNode` calls
/// `LayerNode.make`, which returns `kind: "layer"`. The other two kinds,
/// `"unbound"` and `"group"`, belong to other modules of the layer graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// `kind: "layer"`, a node with an implementation.
    Layer,
}

/// One entry of the `deps` array of `makeGlobalNode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dep {
    /// The context key of the dependency, i.e. `service.key` of the node it
    /// refers to. The node object itself is owned by
    /// [`crate::swarm::location_service_map`] and is not duplicated here.
    pub service: String,
}

/// The `makeGlobalNode({ service, layer, deps })` descriptor.
///
/// `LayerNode.make` returns
/// `{ kind: "layer", name: input.service.key, service, implementation, dependencies, tag }`.
///
/// Two fields are dropped on purpose, and neither is behaviour: `service` is a
/// `Context.Service` class object and `implementation` is the `Layer` itself.
/// Both live in the Effect library's world; what this port keeps is the
/// service **key** and the dependency list, which is what the graph is checked
/// against. The derives are a bridge added for the tests, in the same spirit as
/// the ones on `RunInput`: they let the key and the casing of the dependency
/// names be verified in JSON. The `Layer` in `implementation` is never
/// serialised in the source, because it never leaves memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// Always [`NodeKind::Layer`].
    pub kind: NodeKind,
    /// The service key, `@opencode/v2/SessionExecution`.
    pub service: String,
    /// The tag, `"global"`.
    pub tag: String,
    /// The dependencies, in the order the source lists them.
    pub deps: Vec<Dep>,
}

/// The `export const node = makeGlobalNode({...})` of the source.
///
/// ```ts
/// makeGlobalNode({
///   service: SessionExecution.Service,
///   layer,
///   deps: [SessionStore.node, LocationServiceMap.node],
/// })
/// ```
///
/// The **order** of the two dependencies is the source's and is preserved:
/// the store is what the drain reads, the service map is what it provides.
pub fn node() -> Node {
    Node {
        kind: NodeKind::Layer,
        service: String::from(SERVICE_TAG),
        tag: String::from(NODE_TAG),
        deps: vec![
            Dep {
                service: String::from(SESSION_STORE_TAG),
            },
            Dep {
                service: String::from(LOCATION_SERVICE_MAP_KEY),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::session::execution::{Noop, Service as _};
    use crate::core::session::schema::{Time, Tokens};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// The runner double: records every `RunInput` it is handed.
    ///
    /// `Rc` + `RefCell` is the single-threaded way to let the test read what a
    /// double recorded. Neither is a lock: `Rc` is a non-atomic reference
    /// count with no waiting, `RefCell` borrows and panics immediately on a
    /// re-entrant borrow instead of blocking. There is no `Mutex`, no
    /// `Condvar`, no `Arc`, no thread and no sleep in this file, so no test
    /// here can hang.
    #[derive(Clone, Default)]
    struct Journal(Rc<RefCell<Vec<RunInput>>>);

    impl Journal {
        /// Everything the runner was asked to run, oldest first.
        fn entrees(&self) -> Vec<RunInput> {
            self.0.borrow().clone()
        }
    }

    impl SessionRunner for Journal {
        fn run(&self, input: RunInput) -> Result<(), RunError> {
            self.0.borrow_mut().push(input);
            Ok(())
        }
    }

    /// The runner double that always fails, on the member it was built with.
    struct RunnerEchoue(RunError);

    impl SessionRunner for RunnerEchoue {
        fn run(&self, _input: RunInput) -> Result<(), RunError> {
            Err(self.0)
        }
    }

    /// The service map double: it serves exactly one working directory, and
    /// hands out the same journal every time, so the test sees the calls.
    struct FausseCarte {
        runner: Journal,
        repertoire: String,
    }

    impl FausseCarte {
        /// A map that serves `repertoire`, backed by `runner`.
        fn pour(repertoire: &str, runner: Journal) -> Self {
            Self {
                runner,
                repertoire: repertoire.to_string(),
            }
        }
    }

    impl LocationServices for FausseCarte {
        type Runner = Journal;
        type Error = String;

        fn get(&self, location: &LocationRef) -> Result<Journal, String> {
            if location.directory == self.repertoire {
                Ok(self.runner.clone())
            } else {
                Err(format!("aucun service pour {}", location.directory))
            }
        }
    }

    /// The service map double whose runner always fails.
    struct CarteEchouee;

    impl LocationServices for CarteEchouee {
        type Runner = RunnerEchoue;
        type Error = String;

        fn get(&self, _location: &LocationRef) -> Result<RunnerEchoue, String> {
            Ok(RunnerEchoue(RunError::MessageDecode))
        }
    }

    /// The store double: a list of sessions, plus a record of what it was asked.
    #[derive(Default)]
    struct FauxStore {
        sessions: Vec<SessionInfo>,
        demandes: RefCell<Vec<SessionId>>,
    }

    impl FauxStore {
        fn avec(session: SessionInfo) -> Self {
            Self {
                sessions: vec![session],
                demandes: RefCell::new(Vec::new()),
            }
        }

        fn vide() -> Self {
            Self::default()
        }
    }

    impl SessionStore for FauxStore {
        fn get(&self, session_id: &SessionId) -> Option<SessionInfo> {
            self.demandes.borrow_mut().push(session_id.clone());
            self.sessions
                .iter()
                .find(|session| &session.id == session_id)
                .cloned()
        }
    }

    /// Builds a session. `Info` has twelve fields and no `Default`, so the test
    /// double spells them all out; the empty strings are deliberate, see
    /// `une_session_aux_chaines_vides_est_quand_meme_trouvee`.
    fn session(id: &str, directory: &str) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            parent_id: None,
            project_id: String::from("pro_01"),
            agent: None,
            model: None,
            cost: 0.0,
            tokens: Tokens::default(),
            time: Time::default(),
            title: String::new(),
            location: LocationRef {
                directory: directory.to_string(),
                workspace_id: None,
            },
            subpath: None,
            revert: None,
        }
    }

    fn ref_de(directory: &str) -> LocationRef {
        LocationRef {
            directory: directory.to_string(),
            workspace_id: None,
        }
    }

    /// A store with one session, and a map that serves `/proj/app`.
    fn duo() -> (FauxStore, FausseCarte) {
        (
            FauxStore::avec(session("ses_01", "/proj/app")),
            FausseCarte::pour("/proj/app", Journal::default()),
        )
    }

    // --- wiring ------------------------------------------------------------

    #[test]
    fn les_quatre_membres_du_service_sont_bien_ceux_du_trait_porte() {
        // Cross-check against the ported trait itself: the four names of
        // `Operation::service_member` must be callable on a `SessionExecution`.
        let service = Noop;
        assert!(service.active().is_empty());
        assert!(service.resume("ses_01").is_ok());
        service.wake("ses_01");
        service.interrupt("ses_01");

        let noms: Vec<&str> = OPERATIONS.iter().map(|op| op.service_member()).collect();
        assert_eq!(noms, vec!["active", "interrupt", "resume", "wake"]);
    }

    #[test]
    fn resume_est_le_seul_membre_renomme_en_run() {
        for op in OPERATIONS {
            if op == Operation::Resume {
                assert_eq!(op.service_member(), "resume");
                assert_eq!(op.coordinator_member(), "run");
                assert_ne!(op.service_member(), op.coordinator_member());
            } else {
                assert_eq!(
                    op.service_member(),
                    op.coordinator_member(),
                    "{:?} garde son nom",
                    op
                );
            }
        }
    }

    #[test]
    fn le_tag_du_service_est_celu_de_session_execution() {
        assert_eq!(SERVICE_TAG, "@opencode/v2/SessionExecution");
        // The store is a *different* service; conflating the two would make the
        // dependency list meaningless.
        assert_ne!(SERVICE_TAG, SESSION_STORE_TAG);
        for op in OPERATIONS {
            assert_eq!(op.service_tag(), SERVICE_TAG);
        }
    }

    #[test]
    fn le_cablage_place_chaque_membre_sur_son_membre_du_coordinateur() {
        assert_eq!(
            wiring(),
            vec![
                (Operation::Active, "active", "active"),
                (Operation::Interrupt, "interrupt", "interrupt"),
                (Operation::Resume, "resume", "run"),
                (Operation::Wake, "wake", "wake"),
            ]
        );
    }

    // --- decisions ---------------------------------------------------------

    #[test]
    fn active_photographie_la_carte_quel_que_soit_l_etat_de_la_cle() {
        for etat in [
            None,
            Some(KeyState::default()),
            Some(KeyState::running()),
            Some(KeyState {
                owned: true,
                stopping: true,
                pending_wake: true,
            }),
        ] {
            assert_eq!(action_for(Operation::Active, etat), Action::SnapshotActive);
        }
    }

    #[test]
    fn resume_sur_une_cle_inactive_demarre_une_execution_forcee() {
        assert_eq!(action_for(Operation::Resume, None), Action::StartForced);
    }

    #[test]
    fn resume_sur_une_cle_en_cours_rejoint_l_execution_active() {
        assert_eq!(
            action_for(Operation::Resume, Some(KeyState::running())),
            Action::JoinActive
        );
        // Even with a pending follow-up: `run` waits, it never starts a second
        // execution next to the first one.
        assert_eq!(
            action_for(
                Operation::Resume,
                Some(KeyState {
                    pending_wake: true,
                    ..KeyState::running()
                })
            ),
            Action::JoinActive
        );
    }

    #[test]
    fn resume_sur_une_cle_en_arret_attend_puis_reessaie() {
        assert_eq!(
            action_for(
                Operation::Resume,
                Some(KeyState {
                    owned: true,
                    stopping: true,
                    pending_wake: false
                })
            ),
            Action::AwaitThenRetry
        );
    }

    #[test]
    fn wake_sur_une_cle_inactive_demarre_une_execution_non_forcee() {
        assert_eq!(action_for(Operation::Wake, None), Action::StartUnforced);
    }

    #[test]
    fn wake_sur_une_cle_en_cours_ne_fait_que_coalescer() {
        for etat in [KeyState::default(), KeyState::running()] {
            assert_eq!(action_for(Operation::Wake, Some(etat)), Action::MarkPendingWake);
        }
    }

    #[test]
    fn wake_ne_redemarre_jamais_une_execution_en_cours() {
        // The source returns right after `entry.pendingWake = true`; the
        // successor is started later, by `settle`, when the current one ends.
        let action = action_for(Operation::Wake, Some(KeyState::running()));
        assert_ne!(action, Action::StartUnforced);
        assert_ne!(action, Action::StartForced);
    }

    #[test]
    fn interrupt_sans_entree_est_un_no_op() {
        assert_eq!(action_for(Operation::Interrupt, None), Action::Noop);
    }

    #[test]
    fn interrupt_sur_une_entree_sans_proprietaire_est_un_no_op() {
        // `entry?.owner === undefined` is a NULLITY test: the entry exists,
        // there is simply no fiber to interrupt.
        assert_eq!(
            action_for(
                Operation::Interrupt,
                Some(KeyState {
                    owned: false,
                    stopping: false,
                    pending_wake: true
                })
            ),
            Action::Noop
        );
    }

    #[test]
    fn interrupt_sur_une_entree_possedee_arrete_l_execution() {
        assert_eq!(
            action_for(Operation::Interrupt, Some(KeyState::running())),
            Action::InterruptOwner
        );
        // `entry.pendingWake = false`: the follow-up is dropped with the run.
        assert_eq!(
            action_for(
                Operation::Interrupt,
                Some(KeyState {
                    owned: true,
                    stopping: false,
                    pending_wake: true
                })
            ),
            Action::InterruptOwner
        );
    }

    #[test]
    fn seules_les_actions_de_demarrage_portent_le_drapeau_force() {
        assert_eq!(Action::StartForced.force(), Some(true));
        assert_eq!(Action::StartUnforced.force(), Some(false));
        assert_eq!(Action::MarkPendingWake.force(), Some(false));
        for action in [
            Action::SnapshotActive,
            Action::JoinActive,
            Action::AwaitThenRetry,
            Action::Noop,
            Action::InterruptOwner,
        ] {
            assert_eq!(action.force(), None, "{:?} ne demarre rien", action);
        }
    }

    #[test]
    fn un_reveil_et_une_reprise_ne_transmettent_pas_le_meme_drapeau() {
        let reveil = action_for(Operation::Wake, None);
        let reprise = action_for(Operation::Resume, None);
        assert_eq!(reveil.force(), Some(false));
        assert_eq!(reprise.force(), Some(true));
    }

    #[test]
    fn une_entree_ne_pour_jamais_declencher_deux_demarrages_pour_une_cle() {
        // Exhaustive over the two keys the decisions differ on, so a future
        // edit that adds a starting branch to `wake` or `interrupt` fails here.
        for owned in [false, true] {
            for stopping in [false, true] {
                for pending_wake in [false, true] {
                    let etat = Some(KeyState {
                        owned,
                        stopping,
                        pending_wake,
                    });
                    for operation in OPERATIONS {
                        let demarre = matches!(
                            action_for(operation, etat),
                            Action::StartForced | Action::StartUnforced
                        );
                        assert!(
                            !demarre,
                            "{:?} ne doit jamais demarrer sur une cle deja connue ({:?})",
                            operation,
                            etat
                        );
                    }
                }
            }
        }
    }

    // --- drain -------------------------------------------------------------

    #[test]
    fn le_drain_cherche_la_session_puis_son_repertoire() {
        let (store, carte) = duo();
        let drain = drain(&store, &carte, &String::from("ses_01"), true);

        assert_eq!(store.demandes.borrow().as_slice(), &["ses_01".to_string()]);
        assert_eq!(drain.force(), Some(true));
    }

    #[test]
    fn le_drain_renvoie_au_runner_de_la_location_de_la_session() {
        let (store, carte) = duo();
        let drain = drain(&store, &carte, &String::from("ses_01"), true);

        match drain {
            Drain::Ran {
                location,
                input,
                result,
            } => {
                assert_eq!(location, ref_de("/proj/app"));
                assert_eq!(input, RunInput::new("ses_01", true));
                assert_eq!(result, Ok(()));
            }
            autre => panic!("le drain aurait du joindre le runner, il a rendu {:?}", autre),
        }
    }

    #[test]
    fn le_drain_transmet_le_drapeau_force_sans_le_modifier() {
        let (store, carte) = duo();
        for force in [true, false] {
            let drain = drain(&store, &carte, &String::from("ses_01"), force);
            assert_eq!(drain.force(), Some(force));
        }
    }

    #[test]
    fn le_runner_de_la_location_recoit_exactement_l_entree_de_la_session() {
        let (store, carte) = duo();
        let drain = drain(&store, &carte, &String::from("ses_01"), true);

        assert!(matches!(drain, Drain::Ran { .. }));
        assert_eq!(carte.runner.entrees(), vec![RunInput::new("ses_01", true)]);
    }

    #[test]
    fn un_magasin_vide_rend_toute_cle_absente() {
        let carte = FausseCarte::pour("/proj/app", Journal::default());
        for id in ["ses_01", "", "ses_99"] {
            let drain = drain(&FauxStore::vide(), &carte, &id.to_string(), true);
            assert_eq!(drain, Drain::SessionNotFound(id.to_string()));
        }
    }

    #[test]
    fn une_session_absente_produit_le_defaut_avec_le_message_de_la_source() {
        let (store, carte) = duo();
        let drain = drain(&store, &carte, &String::from("ses_99"), true);

        assert_eq!(drain, Drain::SessionNotFound(String::from("ses_99")));
        assert_eq!(
            drain.defect_message(),
            Some(String::from("Session not found: ses_99"))
        );
        // The runner was never reached, and neither was the Location map.
        assert!(carte.runner.entrees().is_empty());
    }

    #[test]
    fn une_session_aux_chaines_vides_est_quand_meme_trouvee() {
        // `if (!session)` is a TRUTHINESS test on an object. An object is
        // always truthy whatever its fields contain, so a session with an empty
        // id and an empty title is a found session, not a missing one. Porting
        // this as a nullity test on a string would have been wrong.
        let mut vide = session("", "");
        vide.title = String::new();
        let store = FauxStore::avec(vide);
        let carte = FausseCarte::pour("", Journal::default());

        let drain = drain(&store, &carte, &String::from(""), false);
        assert!(matches!(drain, Drain::Ran { .. }));
    }

    #[test]
    fn un_identifiant_vide_arrive_entier_dans_le_message_du_defaut() {
        // Nothing filters `""` before the lookup, and the message interpolates
        // it as it stands: no `??`, no truthiness, no default.
        let (store, carte) = duo();
        let drain = drain(&store, &carte, &String::from(""), true);

        assert_eq!(
            drain.defect_message(),
            Some(String::from("Session not found: "))
        );
    }

    #[test]
    fn une_location_indisponible_atteint_jamais_le_runner() {
        let (store, _carte) = duo();
        let carte = FausseCarte::pour("/ailleurs", Journal::default());

        let drain = drain(&store, &carte, &String::from("ses_01"), true);
        match &drain {
            Drain::LocationUnavailable {
                session_id,
                location,
                error,
            } => {
                assert_eq!(session_id.as_str(), "ses_01");
                assert_eq!(location, &ref_de("/proj/app"));
                assert_eq!(error.as_str(), "aucun service pour /proj/app");
            }
            autre => panic!("le drain aurait du echouer sur la Location, il a rendu {:?}", autre),
        }
        assert!(carte.runner.entrees().is_empty());
    }

    #[test]
    fn un_echec_du_runner_reste_sur_le_canal_type_et_pas_dans_les_defauts() {
        let store = FauxStore::avec(session("ses_01", "/proj/app"));
        let drain = drain(&store, &CarteEchouee, &String::from("ses_01"), true);

        assert_eq!(drain.typed_result(), Ok(Err(RunError::MessageDecode)));
        assert_eq!(drain.cause(), Some(Cause::Failure(RunError::MessageDecode)));
        assert_eq!(drain.defect_message(), None);
    }

    #[test]
    fn un_defaut_et_un_echec_type_ne_se_confondent_pas() {
        let (store, carte) = duo();
        let absent = drain(&store, &carte, &String::from("ses_99"), true);
        assert_eq!(
            absent.typed_result(),
            Err(Defect::SessionNotFound(String::from(
                "Session not found: ses_99"
            )))
        );

        let present = drain(&store, &carte, &String::from("ses_01"), true);
        assert_eq!(present.typed_result(), Ok(Ok(())));
    }

    #[test]
    fn le_defaut_de_la_source_est_lisible_comme_texte() {
        let (store, carte) = duo();
        let drain = drain(&store, &carte, &String::from("ses_99"), true);
        let defaut = drain.typed_result().unwrap_err();
        assert_eq!(defaut.to_string(), "Session not found: ses_99");
    }

    #[test]
    fn la_session_demandee_est_rendue_meme_sans_etre_trouvee() {
        let (store, carte) = duo();
        let absente = drain(&store, &carte, &String::from("ses_99"), true);
        let presente = drain(&store, &carte, &String::from("ses_01"), true);
        let sans_location = drain(
            &FauxStore::avec(session("ses_01", "/ailleurs")),
            &carte,
            &String::from("ses_01"),
            true,
        );

        assert_eq!(absente.session_id(), "ses_99");
        assert_eq!(presente.session_id(), "ses_01");
        assert_eq!(sans_location.session_id(), "ses_01");
    }

    #[test]
    fn un_drain_reussi_n_a_aucune_cause() {
        let (store, carte) = duo();
        let drain = drain(&store, &carte, &String::from("ses_01"), false);
        assert_eq!(drain.cause(), None);
    }

    #[test]
    fn le_drain_est_sans_effet_de_bord_sur_la_cle_recherchee() {
        let (store, carte) = duo();
        let id = String::from("ses_01");
        let premier = drain(&store, &carte, &id, true);
        let second = drain(&store, &carte, &id, true);
        assert_eq!(premier, second);
        assert_eq!(store.demandes.borrow().len(), 2);
    }

    // --- logging -----------------------------------------------------------

    #[test]
    fn une_cause_de_pure_interruption_ne_logue_rien() {
        let session_id = String::from("ses_01");
        let policy = log_policy(&[Cause::Interrupt], &session_id);
        assert_eq!(policy, LogPolicy::Silent);
        assert_eq!(policy.message(), None);
    }

    #[test]
    fn une_cause_qui_melange_interruption_et_echec_est_loguee() {
        // `hasInterruptsOnly` is false as soon as one element is not an
        // interrupt, so the mixed cause takes the log branch.
        let session_id = String::from("ses_01");
        let policy = log_policy(
            &[Cause::Interrupt, Cause::Failure(RunError::Llm)],
            &session_id,
        );
        assert_eq!(policy.message(), Some("Failed to drain Session"));
    }

    #[test]
    fn un_defaut_est_logue_comme_un_echec_type() {
        let session_id = String::from("ses_01");
        let defaut = log_policy(&[Cause::Defect(String::from("boom"))], &session_id);
        let typee = log_policy(&[Cause::Failure(RunError::Llm)], &session_id);
        assert_eq!(defaut, typee);
    }

    #[test]
    fn une_cause_vide_est_loguee_parce_que_la_source_n_en_peut_produire() {
        // `tapCause` only runs on an exit, so the list is never empty. Should it
        // ever be, silence would swallow the error, so the answer is "log".
        let session_id = String::from("ses_01");
        assert!(!has_interrupts_only(&[]));
        assert_eq!(log_policy(&[], &session_id).message(), Some("Failed to drain Session"));
    }

    #[test]
    fn la_cause_renseignee_par_le_drain_pilote_la_decision_de_journalisation() {
        let (store, carte) = duo();
        let id = String::from("ses_99");
        let absent = drain(&store, &carte, &id, true);
        let causes: Vec<Cause> = absent.cause().into_iter().collect();
        assert_eq!(log_policy(&causes, &id).message(), Some("Failed to drain Session"));

        let present = drain(&store, &carte, &String::from("ses_01"), true);
        let causes: Vec<Cause> = present.cause().into_iter().collect();
        assert!(causes.is_empty());
    }

    #[test]
    fn l_annotation_de_journal_serialise_la_cle_en_majuscules() {
        let annotation = LogAnnotation {
            session_id: String::from("ses_01"),
        };
        let json = serde_json::to_string(&annotation).expect("serialisation");
        assert_eq!(json, r#"{"sessionID":"ses_01"}"#);
        assert!(!json.contains("session_id"));
        assert!(!json.contains("sessionId"));
    }

    #[test]
    fn l_annotation_de_journal_refuse_la_forme_minuscule_a_la_relecture() {
        // Both wrong spellings are refused, because the key they use is simply
        // absent and the field is mandatory.
        for faux in [
            r#"{"session_id":"ses_01"}"#,
            r#"{"sessionId":"ses_01"}"#,
            r#"{}"#,
        ] {
            assert!(
                serde_json::from_str::<LogAnnotation>(faux).is_err(),
                "{} aurait du etre refuse",
                faux
            );
        }
        let relu: LogAnnotation =
            serde_json::from_str(r#"{"sessionID":"ses_01"}"#).expect("relecture");
        assert_eq!(relu.session_id, "ses_01");
    }

    #[test]
    fn un_identifiant_vide_survit_dans_l_annotation() {
        // `??` is nullity: `""` is a present value, it is not replaced and it is
        // not dropped. The empty string is therefore written as is.
        let annotation = LogAnnotation {
            session_id: String::new(),
        };
        let json = serde_json::to_string(&annotation).expect("serialisation");
        assert_eq!(json, r#"{"sessionID":""}"#);
        let relu: LogAnnotation = serde_json::from_str(&json).expect("relecture");
        assert_eq!(relu.session_id, "");
    }

    // --- the field names that TypeScript writes with capitals ---------------

    #[test]
    fn l_entree_fabriquee_par_le_drain_serialise_session_id_en_majuscules() {
        let (store, carte) = duo();
        let drain = drain(&store, &carte, &String::from("ses_01"), true);
        let input = match drain {
            Drain::Ran { input, .. } => input,
            autre => panic!("le drain aurait du joindre le runner, il a rendu {:?}", autre),
        };

        let json = serde_json::to_string(&input).expect("serialisation");
        assert_eq!(json, r#"{"sessionID":"ses_01","force":true}"#);
        assert!(!json.contains("sessionId"));
        assert!(!json.contains("session_id"));
    }

    #[test]
    fn l_entree_fabriquee_par_le_drain_refuse_la_forme_minuscule_a_la_relecture() {
        let (store, carte) = duo();
        let drain = drain(&store, &carte, &String::from("ses_01"), true);
        let input = match drain {
            Drain::Ran { input, .. } => input,
            autre => panic!("le drain aurait du joindre le runner, il a rendu {:?}", autre),
        };

        let json = serde_json::to_string(&input).expect("serialisation");
        for faux in [r#""session_id""#, r#""sessionId""#] {
            let cassee = json.replace("\"sessionID\"", faux);
            assert!(
                serde_json::from_str::<RunInput>(&cassee).is_err(),
                "{} aurait du etre refuse dans {}",
                faux,
                cassee
            );
        }
    }

    #[test]
    fn la_session_lue_par_le_drain_porte_son_projet_en_majuscules() {
        let json = serde_json::to_string(&session("ses_01", "/proj/app")).expect("serialisation");
        assert!(json.contains(r#""projectID":"pro_01""#));
        assert!(!json.contains("projectId"));
        assert!(!json.contains("project_id"));

        // Same object, wrong casing: the required `projectID` disappears and
        // the read fails. Writing it in camelCase would compile here and break
        // only at the exchange.
        let cassee = json.replace("\"projectID\"", "\"projectId\"");
        assert!(serde_json::from_str::<SessionInfo>(&cassee).is_err());
    }

    // --- node --------------------------------------------------------------

    #[test]
    fn le_noeud_est_un_noeud_de_couche_global_a_deux_dependances() {
        let n = node();
        assert_eq!(n.kind, NodeKind::Layer);
        assert_eq!(n.service, "@opencode/v2/SessionExecution");
        assert_eq!(n.tag, "global");
        assert_eq!(
            n.deps,
            vec![
                Dep {
                    service: String::from("@opencode/v2/SessionStore")
                },
                Dep {
                    service: String::from("@opencode/example/LocationServiceMap")
                },
            ]
        );
    }

    #[test]
    fn l_ordre_des_dependances_est_celui_de_la_source() {
        // `[SessionStore.node, LocationServiceMap.node]`: the store is read
        // first, the service map is provided afterwards. Swapping them would
        // build a graph that reads as valid and behaves differently.
        let n = node();
        let services: Vec<&str> = n.deps.iter().map(|dep| dep.service.as_str()).collect();
        assert_eq!(
            services,
            vec!["@opencode/v2/SessionStore", "@opencode/example/LocationServiceMap"]
        );
    }

    #[test]
    fn le_noeud_serialise_sa_cle_et_ses_dependances() {
        let json = serde_json::to_string(&node()).expect("serialisation");
        assert!(json.contains(r#""kind":"layer""#));
        assert!(json.contains(r#""tag":"global""#));
        assert!(json.contains("@opencode/v2/SessionExecution"));
        assert!(json.contains("@opencode/v2/SessionStore"));
        assert!(json.contains("@opencode/example/LocationServiceMap"));
    }

    #[test]
    fn le_noeud_refuse_une_dependance_ecrite_avec_une_majuscule() {
        // The field is `service`, in lower case, exactly as in
        // `makeGlobalNode({ service, layer, deps })`.
        let faux = r#"{"kind":"layer","service":"x","tag":"global","deps":[{"Service":"y"}]}"#;
        assert!(serde_json::from_str::<Node>(faux).is_err());

        let bon =
            r#"{"kind":"layer","service":"x","tag":"global","deps":[{"service":"y"}]}"#;
        let relu: Node = serde_json::from_str(bon).expect("relecture");
        assert_eq!(relu.deps, vec![Dep { service: String::from("y") }]);
    }

    #[test]
    fn deux_appels_a_node_donnent_le_meme_noeud() {
        assert_eq!(node(), node());
    }
}
