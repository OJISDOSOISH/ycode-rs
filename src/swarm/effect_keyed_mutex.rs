//! Port of `packages/core/src/effect/keyed-mutex.ts`.
//!
//! The source is a keyed mutual exclusion primitive: one lock per key, and an
//! entry is dropped as soon as no holder and no waiter remain.
//!
//! ```
//! same key      -> queue
//! different key -> run independently
//! ```
//!
//! ## What is ported
//!
//! Every piece of bookkeeping the TypeScript file performs is ported as pure
//! functions over a `BTreeMap<Key, Entry>`:
//!
//! - key extraction and lookup, entry creation (`locks.get` / `locks.set`);
//! - the `users` counter, which counts holders **and** waiters so an entry is
//!   not removed while a waiter is going to reuse it;
//! - what happens when the same key is requested twice: the second request
//!   joins the queue of the first, it never creates a second lock;
//! - what happens for a different key: a separate, fully independent entry;
//! - the release semantics, with their two distinct exits: a holder that
//!   finishes hands the permit to the oldest waiter, and the entry is deleted
//!   only once `users` reaches zero;
//! - `size`, the number of live entries.
//!
//! ## What is NOT ported, and why
//!
//! - **`Semaphore.makeUnsafe(1)` and `withPermit` are not ported.** The permit
//!   counter and the queue that parks a waiting fiber live in the `effect`
//!   library, not in `opencode`. Reproducing them would mean writing a real
//!   lock (`std::sync::Mutex`, `Condvar`, `tokio::sync`), which this port
//!   deliberately does not do. The queue is instead an explicit
//!   `VecDeque<RequestId>` inside `Entry`, and the parking is left to the
//!   caller: `start` returns a `Ticket`, the caller does the work, then calls
//!   `complete` or `interrupt`. Nothing in this module can block, so no test
//!   in this module can hang.
//! - **No unsafe variant.** `makeUnsafe` and `make` differ only by wrapping
//!   the constructor in `Effect.sync`; here both collapse to `new`, and
//!   `make_unsafe` / `make` are kept as aliases for traceability.
//! - **`export * as KeyedMutex from "./keyed-mutex"`** (line 1 of the source)
//!   is a self re-export publishing the module namespace under its own name.
//!   It carries no runtime logic, so there is nothing to translate; in Rust
//!   the module path itself plays that role.
//! - **The scheduling itself** stays with the runtime, exactly as in the
//!   source. Cancellation is modelled explicitly by `interrupt`, but who calls
//!   it, and in which order, is the caller's decision.
//!
//! ## Traps of the source that the port must not get wrong
//!
//! - `Semaphore.makeUnsafe(1)`: the `1` is the **permit count**. A `0` there
//!   would park every holder forever. This port therefore has no permit
//!   counter at all; an entry has exactly one holder slot.
//! - `current ?? { ... }` is a **nullity** test (`undefined` only) while
//!   `if (!current)` is a **truthiness** test. They agree here only because
//!   `Map.get` returns either an object (always truthy) or `undefined`. The
//!   port uses one single `Option` lookup for both, which is sound only
//!   because an `Entry` is never a falsy value.
//! - The entry fields (`semaphore`, `users`) are lower case in this source;
//!   nothing here depends on field-name casing.
//! - `entry.users++` runs when the returned effect **starts**, not when
//!   `withLock(key)(effect)` is called: `Effect.suspend` defers it. The port
//!   mirrors this with `with_lock` (immutable, no side effect) followed by
//!   `start` (the first mutation).
//! - `Effect.ensuring` runs on success, on failure and on interruption, so the
//!   decrement happens exactly once per `start`. `Ticket` is consumed by value
//!   to make "exactly once" a type-level guarantee.

use std::collections::{BTreeMap, VecDeque};

/// Identifier of one acquisition attempt, handed out in call order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

/// One live lock entry: the `users` counter plus the queue guarding the single
/// permit slot. Created empty by `Entry::default`, which is the Rust shape of
/// `Semaphore.makeUnsafe(1)` with `users: 0`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// Acquisitions that started and did not finish: the holder plus every
    /// waiter. The entry is removed from the map only when this hits zero.
    pub users: u32,
    /// The acquisition currently owning the permit slot.
    pub holder: Option<RequestId>,
    /// Acquisitions waiting for the permit, oldest first.
    pub waiters: VecDeque<RequestId>,
}

impl Entry {
    /// Number of acquisitions still waiting for the permit.
    pub fn waiting(&self) -> usize {
        self.waiters.len()
    }

    /// Returns `true` when somebody currently owns the permit.
    pub fn is_held(&self) -> bool {
        self.holder.is_some()
    }

    /// Order in which the outstanding acquisitions will run: the holder first,
    /// then the waiters from the oldest to the newest.
    pub fn order(&self) -> Vec<RequestId> {
        let mut ids = Vec::with_capacity(self.users as usize);
        ids.extend(self.holder);
        ids.extend(self.waiters.iter().copied());
        ids
    }

    /// `users == holder + waiters`. Holds after every operation of this port.
    pub fn is_consistent(&self) -> bool {
        self.users as usize == usize::from(self.holder.is_some()) + self.waiters.len()
    }
}

/// What `start` will do for a key, decided from the map alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    /// No entry for this key: a brand new semaphore is created and the request
    /// owns the permit straight away.
    Fresh,
    /// The entry exists and nobody holds it: the request owns the permit
    /// straight away, behind an already known key.
    Free,
    /// The entry exists and is held: the request joins the queue.
    Queued,
}

/// Pure lookup of `RequestKind`, shared by `KeyedMutex::classify_key` and by
/// the offline schedule simulator.
pub fn classify<K: Ord>(state: &BTreeMap<K, Entry>, key: &K) -> RequestKind {
    match state.get(key) {
        None => RequestKind::Fresh,
        Some(entry) if entry.holder.is_some() => RequestKind::Queued,
        Some(_) => RequestKind::Free,
    }
}

/// Registers one acquisition: `entry.users++`, then either take the free
/// permit or join the queue. Returns `true` when the caller now holds the
/// permit. Creates the entry when the key is unknown, which is the Rust shape
/// of `current ?? { semaphore: Semaphore.makeUnsafe(1), users: 0 }`.
fn acquire_in<K: Ord + Clone>(state: &mut BTreeMap<K, Entry>, key: &K, id: RequestId) -> bool {
    let entry = state.entry(key.clone()).or_default();
    entry.users += 1;
    if entry.holder.is_none() {
        entry.holder = Some(id);
        true
    } else {
        entry.waiters.push_back(id);
        false
    }
}

/// Runs the `Effect.ensuring` bookkeeping: remove the request from the entry,
/// hand the permit to the oldest waiter when the request owned it, decrement
/// `users`, and delete the entry when `users` reaches zero.
///
/// `held` tells whether the request owned the permit when it finished. A
/// request that never owned it (cancelled while queued) is dropped from the
/// queue and hands nothing to anybody.
///
/// Returns `None` when the key has no entry at all. That is unreachable
/// through `KeyedMutex`, which only mints tickets for live entries, but the
/// helper stays total for the offline simulator.
fn release_in<K: Ord>(
    state: &mut BTreeMap<K, Entry>,
    key: &K,
    id: RequestId,
    held: bool,
) -> Option<Completion> {
    let entry = state.get_mut(key)?;
    let _still_queued = entry.waiters.remove(&id);
    let mut handed_over = None;
    if held && entry.holder == Some(id) {
        entry.holder = entry.waiters.pop_front();
        handed_over = entry.holder;
    }
    entry.users = entry.users.saturating_sub(1);
    let removed = entry.users == 0;
    let users_left = entry.users;
    if removed {
        state.remove(key);
    }
    Some(Completion {
        removed,
        handed_over,
        users_left,
    })
}

/// Result of one `complete` / `interrupt` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    /// `true` when the entry was deleted from the map, that is when `users`
    /// reached zero and no holder nor waiter remains.
    pub removed: bool,
    /// The waiter promoted to holder, if any. `None` when the finishing
    /// request did not own the permit.
    pub handed_over: Option<RequestId>,
    /// `users` left on the entry after the decrement; always `0` when removed.
    pub users_left: u32,
}

/// One step of a deterministic schedule, replayed by [`simulate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step<K> {
    /// `with_lock(key)` then `start`: the first mutation for this request.
    Start(K),
    /// `complete` for the acquisition that received this id.
    Complete(RequestId),
    /// `interrupt` for the acquisition that received this id.
    Interrupt(RequestId),
}

/// What one replayed step did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepReport {
    /// An acquisition started.
    Started {
        /// Id given to the new acquisition.
        id: RequestId,
        /// State of the key before the acquisition.
        kind: RequestKind,
        /// `true` when the acquisition owns the permit.
        holding: bool,
    },
    /// An acquisition finished and released the entry.
    Finished {
        /// Id of the acquisition that finished.
        id: RequestId,
        /// Outcome of the release bookkeeping.
        completed: Completion,
    },
    /// The id was not outstanding. Unreachable in a well formed schedule.
    Unknown {
        /// Id referenced by the step.
        id: RequestId,
    },
}

/// Result of [`simulate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Simulation<K> {
    /// Final state of the map.
    pub state: BTreeMap<K, Entry>,
    /// One report per step, in schedule order.
    pub reports: Vec<StepReport>,
}

/// Replays a schedule on a copy of `initial`, without touching any lock and
/// without blocking: the schedule is a finite slice and every step is applied
/// once, in order.
pub fn simulate<K: Ord + Clone>(initial: &BTreeMap<K, Entry>, schedule: &[Step<K>]) -> Simulation<K> {
    let mut state = initial.clone();
    let mut pending: BTreeMap<RequestId, (K, bool)> = BTreeMap::new();
    let mut next_id: u64 = 0;
    let mut reports = Vec::with_capacity(schedule.len());

    for step in schedule {
        match step.clone() {
            Step::Start(key) => {
                let id = RequestId(next_id);
                next_id += 1;
                let kind = classify(&state, &key);
                let holding = acquire_in(&mut state, &key, id);
                pending.insert(id, (key.clone(), holding));
                reports.push(StepReport::Started { id, kind, holding });
            }
            Step::Complete(id) | Step::Interrupt(id) => match pending.remove(&id) {
                None => reports.push(StepReport::Unknown { id }),
                Some((key, holding)) => {
                    let completed =
                        release_in(&mut state, &key, id, holding).expect("entry of a pending id");
                    reports.push(StepReport::Finished { id, completed });
                }
            },
        }
    }

    Simulation { state, reports }
}

/// A keyed mutex with one lock per key, the port of `makeUnsafe<Key>()`.
///
/// The value is inert until used, and every method that changes the map takes
/// `&mut self`, so exclusive access is a borrow-checker guarantee rather than
/// a runtime lock.
#[derive(Debug)]
pub struct KeyedMutex<K: Ord> {
    state: BTreeMap<K, Entry>,
    next_id: u64,
    outstanding: BTreeMap<RequestId, K>,
}

impl<K: Ord + Clone> KeyedMutex<K> {
    /// Creates an empty keyed mutex. This is both `makeUnsafe<Key>()` and
    /// `make<Key>()` from the source.
    pub fn new() -> Self {
        Self {
            state: BTreeMap::new(),
            next_id: 0,
            outstanding: BTreeMap::new(),
        }
    }

    /// Alias of [`KeyedMutex::new`], mirroring the source `make`.
    pub fn make() -> Self {
        Self::new()
    }

    /// Alias of [`KeyedMutex::new`], mirroring the source `makeUnsafe`.
    pub fn make_unsafe() -> Self {
        Self::new()
    }

    /// `withLock(key)`: describes the effect to run under the lock. It borrows
    /// the mutex immutably and changes nothing, because `Effect.suspend`
    /// defers the whole bookkeeping to the moment the effect starts.
    pub fn with_lock(&self, key: K) -> Pipeline<K> {
        Pipeline { key }
    }

    /// Starts the pipeline: creates the entry if the key is unknown, counts one
    /// user, and either takes the permit or joins the queue.
    pub fn start(&mut self, pipeline: &Pipeline<K>) -> Ticket<K> {
        let id = RequestId(self.next_id);
        self.next_id += 1;
        let holding = acquire_in(&mut self.state, &pipeline.key, id);
        self.outstanding.insert(id, pipeline.key.clone());
        Ticket {
            key: pipeline.key.clone(),
            id,
            state: if holding {
                TicketState::Holding
            } else {
                TicketState::Waiting
            },
        }
    }

    /// `size`: the number of live entries.
    pub fn size(&self) -> usize {
        self.state.len()
    }

    /// Returns `true` when no entry is alive.
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    /// Returns `true` when an entry exists for `key`.
    pub fn contains(&self, key: &K) -> bool {
        self.state.contains_key(key)
    }

    /// Borrows the entry of `key`, if any.
    pub fn entry(&self, key: &K) -> Option<&Entry> {
        self.state.get(key)
    }

    /// `users` for `key`, holders and waiters counted together.
    pub fn users(&self, key: &K) -> Option<u32> {
        self.state.get(key).map(|entry| entry.users)
    }

    /// The acquisition currently owning the permit of `key`.
    pub fn holder(&self, key: &K) -> Option<RequestId> {
        self.state.get(key).and_then(|entry| entry.holder)
    }

    /// The queue of `key`, oldest waiter first.
    pub fn waiters(&self, key: &K) -> Vec<RequestId> {
        self.state
            .get(key)
            .map(|entry| entry.waiters.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Pure classification of `key` against the current map.
    pub fn classify_key(&self, key: &K) -> RequestKind {
        classify(&self.state, key)
    }

    /// Live keys, in map order, so the listing is deterministic.
    pub fn live_keys(&self) -> Vec<K> {
        self.state.keys().cloned().collect()
    }

    /// Number of acquisitions that started and did not finish yet.
    pub fn outstanding(&self) -> usize {
        self.outstanding.len()
    }

    /// Returns `true` when every entry satisfies `users == holder + waiters`
    /// and the number of outstanding tickets matches the sum of `users`.
    pub fn is_consistent(&self) -> bool {
        let counted: u32 = self.state.values().map(|entry| entry.users).sum();
        self.state.values().all(Entry::is_consistent)
            && counted as usize == self.outstanding.len()
    }

    /// Runs the work under the lock, then finishes the ticket.
    ///
    /// This is the `Effect.ensuring` half of the source: the permit goes back
    /// to the oldest waiter, `users` is decremented, and the entry is deleted
    /// when no user remains.
    pub fn complete(&mut self, ticket: Ticket<K>) -> Option<Completion> {
        self.finish(ticket)
    }

    /// Finishes a ticket that was interrupted.
    ///
    /// Identical bookkeeping to [`KeyedMutex::complete`], and deliberately so:
    /// `Effect.ensuring` runs on interruption too, and a holder that is
    /// interrupted while holding the permit still returns it. The only
    /// difference is that a waiter which never acquired the permit leaves the
    /// queue without passing anything on, which `release_in` decides from the
    /// ticket state alone.
    pub fn interrupt(&mut self, ticket: Ticket<K>) -> Option<Completion> {
        self.finish(ticket)
    }

    fn finish(&mut self, ticket: Ticket<K>) -> Option<Completion> {
        let held = ticket.state == TicketState::Holding;
        self.outstanding.remove(&ticket.id);
        release_in(&mut self.state, &ticket.key, ticket.id, held)
    }
}

impl<K: Ord + Clone> Default for KeyedMutex<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// The lazy description of an effect to run under a key, built by
/// [`KeyedMutex::with_lock`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pipeline<K> {
    key: K,
}

impl<K> Pipeline<K> {
    /// Key this pipeline locks.
    pub fn key(&self) -> &K {
        &self.key
    }
}

/// Whether a started ticket owns the permit or is still queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TicketState {
    /// The ticket owns the single permit slot of its entry.
    Holding,
    /// The ticket is queued behind the current holder.
    Waiting,
}

/// One started acquisition. Consumed by value exactly once, which reproduces
/// the "exactly one `ensuring` per `start`" guarantee of the source.
#[derive(Debug)]
pub struct Ticket<K> {
    key: K,
    id: RequestId,
    state: TicketState,
}

impl<K> Ticket<K> {
    /// Identifier of this acquisition.
    pub fn id(&self) -> RequestId {
        self.id
    }

    /// Key this ticket locks.
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Whether the permit is owned or still queued.
    pub fn state(&self) -> TicketState {
        self.state
    }

    /// Returns `true` when this ticket owns the permit.
    pub fn holds_permit(&self) -> bool {
        self.state == TicketState::Holding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(mutex: &mut KeyedMutex<String>, key: &str) -> Ticket<String> {
        let pipeline = mutex.with_lock(key.to_string());
        mutex.start(&pipeline)
    }

    #[test]
    fn a_new_mutex_is_empty() {
        let mutex = KeyedMutex::<String>::new();

        assert_eq!(mutex.size(), 0);
        assert!(mutex.is_empty());
        assert!(mutex.live_keys().is_empty());
        assert_eq!(mutex.outstanding(), 0);
        assert_eq!(mutex.classify_key(&"a".to_string()), RequestKind::Fresh);
        assert_eq!(mutex.users(&"a".to_string()), None);
        assert_eq!(mutex.holder(&"a".to_string()), None);
    }

    #[test]
    fn make_and_make_unsafe_are_the_same_constructor() {
        assert_eq!(KeyedMutex::<String>::make().size(), 0);
        assert_eq!(KeyedMutex::<String>::make_unsafe().size(), 0);
        assert_eq!(KeyedMutex::<String>::default().size(), 0);
    }

    #[test]
    fn building_a_pipeline_touches_nothing() {
        let mutex = KeyedMutex::<String>::new();

        let pipeline = mutex.with_lock("a".to_string());

        assert_eq!(pipeline.key(), "a");
        assert_eq!(mutex.size(), 0);
        assert!(mutex.is_empty());
        assert!(!mutex.contains(&"a".to_string()));
    }

    #[test]
    fn starting_an_unknown_key_creates_the_entry_with_one_user() {
        let mut mutex = KeyedMutex::<String>::new();

        let ticket = start(&mut mutex, "a");

        assert_eq!(ticket.id(), RequestId(0));
        assert_eq!(ticket.state(), TicketState::Holding);
        assert!(ticket.holds_permit());
        assert_eq!(mutex.size(), 1);
        assert_eq!(mutex.users(&"a".to_string()), Some(1));
        assert_eq!(mutex.holder(&"a".to_string()), Some(RequestId(0)));
        assert!(mutex.is_consistent());
    }

    #[test]
    fn the_same_key_requested_twice_queues_the_second_request() {
        let mut mutex = KeyedMutex::<String>::new();

        let first = start(&mut mutex, "a");
        let second = start(&mut mutex, "a");

        assert_eq!(second.state(), TicketState::Waiting);
        assert!(!second.holds_permit());
        assert_eq!(mutex.size(), 1);
        assert_eq!(mutex.users(&"a".to_string()), Some(2));
        assert_eq!(mutex.holder(&"a".to_string()), Some(first.id()));
        assert_eq!(mutex.waiters(&"a".to_string()), vec![second.id()]);
        assert_eq!(mutex.classify_key(&"a".to_string()), RequestKind::Queued);
        assert!(mutex.is_consistent());
    }

    #[test]
    fn different_keys_are_completely_independent() {
        let mut mutex = KeyedMutex::<String>::new();

        let a = start(&mut mutex, "a");
        let b = start(&mut mutex, "b");

        assert!(a.holds_permit());
        assert!(b.holds_permit());
        assert_eq!(mutex.size(), 2);
        assert_eq!(mutex.live_keys(), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(mutex.users(&"a".to_string()), Some(1));
        assert_eq!(mutex.users(&"b".to_string()), Some(1));
        assert!(mutex.is_consistent());
    }

    #[test]
    fn completing_the_holder_hands_the_permit_to_the_oldest_waiter() {
        let mut mutex = KeyedMutex::<String>::new();

        let first = start(&mut mutex, "a");
        let second = start(&mut mutex, "a");
        let third = start(&mut mutex, "a");

        let report = mutex.complete(first).expect("live entry");

        assert_eq!(report.handed_over, Some(second.id()));
        assert!(!report.removed);
        assert_eq!(mutex.holder(&"a".to_string()), Some(second.id()));
        assert_eq!(mutex.waiters(&"a".to_string()), vec![third.id()]);
        assert_eq!(mutex.size(), 1);
        assert!(mutex.is_consistent());
    }

    #[test]
    fn the_entry_survives_while_a_waiter_remains() {
        let mut mutex = KeyedMutex::<String>::new();

        let first = start(&mut mutex, "a");
        let second = start(&mut mutex, "a");

        let report = mutex.complete(first).expect("live entry");

        assert_eq!(report.users_left, 1);
        assert!(!report.removed);
        assert!(mutex.contains(&"a".to_string()));
        assert_eq!(mutex.size(), 1);
        assert_eq!(mutex.users(&"a".to_string()), Some(1));
    }

    #[test]
    fn the_entry_is_removed_when_the_last_user_finishes() {
        let mut mutex = KeyedMutex::<String>::new();

        let first = start(&mut mutex, "a");
        let second = start(&mut mutex, "a");

        mutex.complete(first);
        let report = mutex.complete(second).expect("still live");

        assert!(report.removed);
        assert_eq!(report.users_left, 0);
        assert_eq!(report.handed_over, None);
        assert!(!mutex.contains(&"a".to_string()));
        assert_eq!(mutex.size(), 0);
        assert_eq!(mutex.users(&"a".to_string()), None);
        assert_eq!(mutex.outstanding(), 0);
        assert!(mutex.is_consistent());
    }

    #[test]
    fn a_reused_key_gets_a_brand_new_entry() {
        let mut mutex = KeyedMutex::<String>::new();

        let first = start(&mut mutex, "a");
        mutex.complete(first);
        assert_eq!(mutex.classify_key(&"a".to_string()), RequestKind::Fresh);

        let second = start(&mut mutex, "a");

        assert!(second.holds_permit());
        assert_eq!(mutex.classify_key(&"a".to_string()), RequestKind::Free);
        let entry = mutex.entry(&"a".to_string()).expect("new entry");
        assert_eq!(entry.waiters.len(), 0);
        assert_eq!(entry.users, 1);
    }

    #[test]
    fn an_interrupted_waiter_leaves_the_queue_without_a_handover() {
        let mut mutex = KeyedMutex::<String>::new();

        let holder = start(&mut mutex, "a");
        let waiter = start(&mut mutex, "a");

        let report = mutex.interrupt(waiter).expect("live entry");

        assert_eq!(report.handed_over, None);
        assert!(!report.removed);
        assert_eq!(report.users_left, 1);
        assert_eq!(mutex.waiters(&"a".to_string()), Vec::new());
        assert_eq!(mutex.holder(&"a".to_string()), Some(holder.id()));
        assert!(mutex.is_consistent());
    }

    #[test]
    fn an_interrupted_holder_still_returns_the_permit() {
        let mut mutex = KeyedMutex::<String>::new();

        let holder = start(&mut mutex, "a");
        let waiter = start(&mut mutex, "a");

        let report = mutex.interrupt(holder).expect("live entry");

        assert_eq!(report.handed_over, Some(waiter.id()));
        assert!(!report.removed);
        assert_eq!(mutex.holder(&"a".to_string()), Some(waiter.id()));
        assert!(mutex.is_consistent());
    }

    #[test]
    fn two_keys_keep_their_own_queues() {
        let mut mutex = KeyedMutex::<String>::new();

        let a1 = start(&mut mutex, "a");
        let a2 = start(&mut mutex, "a");
        let b1 = start(&mut mutex, "b");

        mutex.complete(a1);
        mutex.complete(b1);

        assert_eq!(mutex.live_keys(), vec!["a".to_string()]);
        assert_eq!(mutex.users(&"a".to_string()), Some(1));
        assert_eq!(mutex.users(&"b".to_string()), None);
        assert_eq!(mutex.holder(&"a".to_string()), Some(a2.id()));
        assert!(mutex.is_consistent());
    }

    #[test]
    fn entry_order_is_holder_then_oldest_waiter() {
        let mut mutex = KeyedMutex::<String>::new();

        let first = start(&mut mutex, "a");
        let second = start(&mut mutex, "a");
        let third = start(&mut mutex, "a");

        let entry = mutex.entry(&"a".to_string()).expect("live entry");

        assert_eq!(entry.order(), vec![first.id(), second.id(), third.id()]);
        assert_eq!(entry.waiting(), 2);
        assert!(entry.is_held());
        assert!(entry.is_consistent());
    }

    #[test]
    fn classify_reads_the_map_without_mutating_it() {
        let mut mutex = KeyedMutex::<String>::new();
        let key = "a".to_string();

        assert_eq!(classify(&mutex.state, &key), RequestKind::Fresh);
        let ticket = start(&mut mutex, "a");
        assert_eq!(classify(&mutex.state, &key), RequestKind::Queued);
        mutex.complete(ticket);
        assert_eq!(classify(&mutex.state, &key), RequestKind::Free);
    }

    #[test]
    fn simulate_replays_a_schedule_without_touching_a_live_mutex() {
        let initial: BTreeMap<String, Entry> = BTreeMap::new();
        let schedule = vec![
            Step::Start("a".to_string()),
            Step::Start("a".to_string()),
            Step::Start("b".to_string()),
            Step::Complete(RequestId(0)),
            Step::Complete(RequestId(2)),
            Step::Complete(RequestId(1)),
        ];

        let result = simulate(&initial, &schedule);

        assert_eq!(
            result.reports,
            vec![
                StepReport::Started {
                    id: RequestId(0),
                    kind: RequestKind::Fresh,
                    holding: true,
                },
                StepReport::Started {
                    id: RequestId(1),
                    kind: RequestKind::Queued,
                    holding: false,
                },
                StepReport::Started {
                    id: RequestId(2),
                    kind: RequestKind::Fresh,
                    holding: true,
                },
                StepReport::Finished {
                    id: RequestId(0),
                    completed: Completion {
                        removed: false,
                        handed_over: Some(RequestId(1)),
                        users_left: 1,
                    },
                },
                StepReport::Finished {
                    id: RequestId(2),
                    completed: Completion {
                        removed: true,
                        handed_over: None,
                        users_left: 0,
                    },
                },
                StepReport::Finished {
                    id: RequestId(1),
                    completed: Completion {
                        removed: true,
                        handed_over: None,
                        users_left: 0,
                    },
                },
            ]
        );
        assert!(result.state.is_empty());
    }

    #[test]
    fn simulate_starts_from_a_snapshot_and_leaves_it_untouched() {
        let mut mutex = KeyedMutex::<String>::new();
        let holder = start(&mut mutex, "a");
        let snapshot = mutex.state.clone();

        let result = simulate(
            &snapshot,
            &[Step::Start("a".to_string()), Step::Complete(RequestId(0))],
        );

        assert_eq!(result.reports[0], StepReport::Started {
            id: RequestId(0),
            kind: RequestKind::Queued,
            holding: false,
        });
        assert_eq!(result.state.len(), 1);
        assert_eq!(mutex.users(&"a".to_string()), Some(1));
        assert!(mutex.waiters(&"a".to_string()).is_empty());
        assert_eq!(mutex.holder(&"a".to_string()), Some(holder.id()));
        assert!(mutex.is_consistent());
    }

    #[test]
    fn simulate_reports_an_unknown_id() {
        let initial: BTreeMap<String, Entry> = BTreeMap::new();

        let result = simulate(&initial, &[Step::Complete(RequestId(7))]);

        assert_eq!(result.reports, vec![StepReport::Unknown { id: RequestId(7) }]);
    }

    #[test]
    fn every_entry_stays_consistent_along_a_long_schedule() {
        let initial: BTreeMap<String, Entry> = BTreeMap::new();
        let schedule = vec![
            Step::Start("a".to_string()),
            Step::Start("b".to_string()),
            Step::Start("a".to_string()),
            Step::Interrupt(RequestId(1)),
            Step::Complete(RequestId(0)),
            Step::Start("a".to_string()),
            Step::Start("c".to_string()),
            Step::Complete(RequestId(2)),
            Step::Complete(RequestId(4)),
            Step::Complete(RequestId(3)),
            Step::Complete(RequestId(5)),
        ];

        let result = simulate(&initial, &schedule);

        assert!(result.state.is_empty());
        assert!(result
            .state
            .values()
            .all(|entry| entry.is_consistent()));
    }
}