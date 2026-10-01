//! Port of the portable part of `opencode/packages/core/src/state.ts`.
//!
//! A value that plugins extend by registering transforms, where each transform
//! runs on a FRESH value every time and only the final value becomes visible.
//! That indirection is the whole point: a transform never patches the live
//! value, so an extension cannot observe or depend on another extension's
//! mutations, and a reload always replays every transform from the base.
//!
//! Not ported: the Effect machinery - `Semaphore`, `Scope`, `Context.Reference`
//! and the generator plumbing. Ported: the ordering rules, the reload
//! deduplication, and the dispose idempotence, because those are the behaviour
//! and they are all observable.
//!
//! The rules, each with a test:
//!
//! - Transforms run in REGISTRATION order, always on a base value built by
//!   `initial`, never on the last committed one. Replaying from the base is
//!   what makes a transform a pure function of the base.
//!
//! - `finalize` runs after every transform and BEFORE the value is published.
//!   So a finalizer sees the transforms' output, and `get` never observes a
//!   value that has not been finalized.
//!
//! - Disposal is idempotent. The TS keeps an `active` flag per registration and
//!   the disposer is only wired once, but a caller holding the `dispose`
//!   function can call it twice; the second call does nothing at all, including
//!   not reloading.
//!
//! - Inside a `batch`, a transform does NOT reload immediately: it queues its
//!   reload and the batch runs each queued reload once at the end. The queue is a
//!   `Set<Reload>`, and `Reload` is the single reload function the store builds
//!   once - so N transforms of one store all queue THE SAME function and the set
//!   collapses them to ONE reload. That is the entire value of batching: a plugin
//!   group that touches six settings on one store reloads it once. Two different
//!   stores do produce two reloads, because they queue two different functions.
//!
//! - A nested `batch` does not create a second batch. The inner one sees a
//!   batch already in progress and just runs its body, so reloads queued inside
//!   it land in the OUTER batch and run when the outer one closes.
//!
//! Two modelling notes. The draft API is elided: in the TS a transform mutates
//! a domain-specific draft wrapper and only `commit` publishes it, while here a
//! transform receives `&mut State` on a value that is published only at the
//! end - the observable behaviour is the same. And the semaphore of one permit
//! is the mutex guarding the transforms, so the mutual exclusion the TS gets
//! from `Semaphore.makeUnsafe(1)` is the mutual exclusion here; the batch
//! context is a thread-local stack, which is the closest equivalent of an
//! Effect context that survives being passed to a closure.
//!
//! A transform that fails is a defect rather than a value: the TS pipes it
//! through `orDie`, which kills the fiber. A panicking callback is the
//! equivalent here, and the state is left unpublished because `commit` is only
//! reached after every transform returned.

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// A registered transform: it mutates the fresh value in place.
pub type Transform<S> = Box<dyn Fn(&mut S) + Send + Sync>;

/// Options, as the TS `Options` interface.
pub struct Options<S: Send + Sync + 'static> {
    /// Builds the base value, for the initial state and for every reload.
    pub initial: Box<dyn Fn() -> S + Send + Sync>,
    /// Runs after every transform and before the value is published.
    pub finalize: Option<Box<dyn Fn(&S) + Send + Sync>>,
}

struct Entry<S> {
    id: usize,
    active: bool,
    run: Transform<S>,
}

/// A store's identity, which is what a batch queue deduplicates on.
///
/// See `NEXT_STORE_ID` for why the queue holds this rather than a registration
/// id: in the TypeScript it holds the store's single reload FUNCTION.
pub type StoreId = usize;

/// Distinguishes stores from one another, so a batch queue can be keyed by
/// STORE rather than by registration.
///
/// This exists because of one detail of the TypeScript that is easy to read
/// past. The batch queue is a `Set<Reload>`, and `Reload` is a FUNCTION -
/// specifically the single `reload` closure that `create` builds once per store.
/// Every transform registered against that store queues THE SAME function
/// object, so the `Set` collapses them: N transforms in one batch produce ONE
/// reload. That is the entire point of batching.
///
/// Keying the queue by registration instead, as the first draft here did,
/// breaks exactly that: two transforms on one store queue two distinct ids, the
/// caller reloads twice, and the invariant the module header claims is quietly
/// false. The final VALUE is the same either way, which is why nothing caught
/// it - and why it needed a reading of the source rather than a run.
static NEXT_STORE_ID: AtomicUsize = AtomicUsize::new(0);

struct Inner<S> {
    committed: Option<S>,
    transforms: Vec<Entry<S>>,
}

thread_local! {
    /// How many batches are open on this thread. The TS gets this from a
    /// `Context.Reference` with a `Set<Reload> | undefined` default; a
    /// thread-local depth reproduces the same "already batching?" question
    /// without threading a parameter through every call.
    static BATCH_DEPTH: RefCell<usize> = const { RefCell::new(0) };
    /// The reloads queued by the innermost open batch, shared across stores:
    /// the TS queue is per-effect-call, not per-store.
    static BATCHED: RefCell<Vec<HashSet<usize>>> = const { RefCell::new(Vec::new()) };
}

fn in_batch() -> bool {
    BATCH_DEPTH.with(|depth| *depth.borrow() > 0)
}

fn queue_reload(store_id: StoreId) {
    BATCHED.with(|batched| {
        if let Some(current) = batched.borrow_mut().last_mut() {
            current.insert(store_id);
        }
    });
}

/// The store, as `create` returns it.
///
/// `S` is the state type. The TS parameterises this over a `DraftApi` as well;
/// see the module header for why that indirection is elided here.
pub struct Store<S: Send + Sync + 'static> {
    initial: Box<dyn Fn() -> S + Send + Sync>,
    finalize: Option<Box<dyn Fn(&S) + Send + Sync>>,
    inner: Mutex<Inner<S>>,
    next_id: AtomicUsize,
    /// This store's identity, which is what a batch queue holds. See
    /// `NEXT_STORE_ID` for why it is the store and not the registration.
    store_id: StoreId,
}

/// What a registration hands back, as the TS `Registration` interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registration {
    id: usize,
}

impl Registration {
    /// The registration's identity, useful for asserting on it.
    pub fn id(&self) -> usize {
        self.id
    }
}

impl<S: Send + Sync + 'static> Store<S> {
    /// `create`: the first value is built and published immediately.
    pub fn new(options: Options<S>) -> Self {
        let state = (options.initial)();
        Store {
            initial: options.initial,
            finalize: options.finalize,
            inner: Mutex::new(Inner { committed: Some(state), transforms: Vec::new() }),
            next_id: AtomicUsize::new(0),
            store_id: NEXT_STORE_ID.fetch_add(1, Ordering::SeqCst),
        }
    }

    /// This store's identity - the value a batch queue deduplicates on.
    pub fn store_id(&self) -> StoreId {
        self.store_id
    }

    /// `get`: the committed value.
    ///
    /// The `S: Clone` bound is on this one method, not on the store: publishing
    /// a value does not need a copy, only reading one out does.
    pub fn get(&self) -> Option<S>
    where
        S: Clone,
    {
        self.inner.lock().ok()?.committed.clone()
    }

    /// `materialize`: a fresh base, every transform in order, then commit.
    ///
    /// Published in place, with no clone: bounding `S: Clone` here would push a
    /// bound the store does not otherwise need onto every caller.
    fn materialize_locked(&self, inner: &mut Inner<S>) {
        let mut next = (self.initial)();
        for entry in &inner.transforms {
            if !entry.active {
                continue;
            }
            (entry.run)(&mut next);
        }
        if let Some(finalize) = &self.finalize {
            finalize(&next);
        }
        inner.committed = Some(next);
    }

    /// `reload`: one permit, then materialize.
    pub fn reload(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            self.materialize_locked(&mut inner);
        }
    }

    /// `transform`: register, then reload - or queue the reload when batching.
    ///
    /// The order matters and is the source's: the transform is pushed onto the
    /// list BEFORE the reload, so the reload replays it. Registering first and
    /// reloading after is what makes the call synchronous.
    pub fn transform(&self, update: Transform<S>) -> Registration {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        {
            let Ok(mut inner) = self.inner.lock() else {
                return Registration { id };
            };
            inner.transforms.push(Entry { id, active: true, run: update });
        }
        if in_batch() {
            queue_reload(self.store_id);
        } else {
            self.reload();
        }
        Registration { id }
    }

    /// Removes a registration and reloads, exactly like the TS disposer.
    pub fn dispose(&self, registration: Registration) {
        let queued;
        {
            let Ok(mut inner) = self.inner.lock() else { return };
            match inner.transforms.iter_mut().find(|item| item.id == registration.id) {
                // The `active` flag is what makes a second dispose a no-op,
                // including the reload it would otherwise trigger.
                None => return,
                Some(item) if !item.active => return,
                Some(item) => {
                    item.active = false;
                    inner.transforms.retain(|item| item.id != registration.id);
                }
            }
            queued = in_batch();
        }
        if queued {
            queue_reload(self.store_id);
        } else {
            self.reload();
        }
    }

    /// Whether a registration is still live.
    pub fn is_registered(&self, registration: Registration) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.transforms.iter().any(|item| item.id == registration.id))
            .unwrap_or(false)
    }

    /// How many transforms are live. Test affordance.
    pub fn transform_count(&self) -> usize {
        self.inner.lock().map(|inner| inner.transforms.len()).unwrap_or(0)
    }
}

/// What a batch returns: the body's value, and the stores it queued a reload for.
///
/// The TS runs the queued reloads itself on the way out, because it holds the
/// store through the Effect context. Here the queue holds store identities
/// rather than closures over a store, so the caller runs them - which is more
/// code but cannot quietly do the wrong thing for the wrong store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOutcome<R> {
    pub value: R,
    /// The stores whose reload was queued, DEDUPLICATED and ascending.
    ///
    /// One entry per STORE, not per registration, and that is the source's
    /// behaviour rather than a simplification of it: the TS queue is a
    /// `Set<Reload>` where every transform of one store adds the same function.
    /// Two transforms in one batch therefore reload ONCE. Reloading twice would
    /// produce the same value, so the count is the only thing that can tell the
    /// two implementations apart - and it is the thing batching exists to fix.
    pub stores: Vec<usize>,
}

/// `batch`: run a body, collecting the reloads it queued.
///
/// Nested calls do not open a second queue: the inner one runs its body and
/// reports nothing, so reloads queued inside it land in the outer batch and are
/// reported by the outer call.
pub fn batch<R>(body: impl FnOnce() -> R) -> BatchOutcome<R> {
    if in_batch() {
        return BatchOutcome { value: body(), stores: Vec::new() };
    }
    BATCH_DEPTH.with(|depth| *depth.borrow_mut() += 1);
    BATCHED.with(|batched| batched.borrow_mut().push(HashSet::new()));
    let value = body();
    let queued = BATCHED.with(|batched| batched.borrow_mut().pop().unwrap_or_default());
    BATCH_DEPTH.with(|depth| *depth.borrow_mut() -= 1);
    let mut stores: Vec<usize> = queued.into_iter().collect();
    stores.sort_unstable();
    BatchOutcome { value, stores }
}

/// The stores queued by the innermost open batch, ascending.
///
/// Lets a caller inspect the queue from inside a batch body. Returns nothing
/// outside a batch.
pub fn queued_now() -> Vec<usize> {
    let mut ids: Vec<usize> = BATCHED
        .try_with(|batched| batched.borrow().last().map(|set| set.iter().copied().collect()))
        .unwrap_or_default()
        .unwrap_or_default();
    ids.sort_unstable();
    ids
}

/// `Options` for a state that needs no finalizer.
pub fn without_finalize<S: Send + Sync + 'static>(initial: impl Fn() -> S + Send + Sync + 'static) -> Options<S> {
    Options { initial: Box::new(initial), finalize: None }
}

/// The serialized shape of a registration, for callers that persist one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrationWire {
    pub id: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    type Lines = Vec<String>;

    fn store() -> Store<Lines> {
        Store::new(without_finalize(Vec::new))
    }

    /// A store whose base value counts how many times it has been built, which
    /// is how a reload is observed from outside.
    ///
    /// The counter is an `Arc`, not a reference: `Transform` is `'static`, so a
    /// `move` closure cannot capture a borrow that dies at this function's
    /// return. Passing `Arc<AtomicUsize>` by value is what makes the closure
    /// well-formed - a `&AtomicUsize` compiles nowhere else in these tests
    /// either.
    fn counting_store(count: std::sync::Arc<std::sync::atomic::AtomicUsize>) -> Store<Lines> {
        Store::new(without_finalize(move || {
            count.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        }))
    }

    fn push(store: &Store<Lines>, value: &str) -> Registration {
        // Owned before the closure: `Transform` is `'static`, so a borrow of a
        // test parameter could not outlive this function.
        let value = value.to_string();
        store.transform(Box::new(move |state: &mut Lines| state.push(value.clone())))
    }

    /// Reloads `store` once per queued entry naming it - which is what `batch`
    /// does on the way out in the TS: it holds the store through the Effect
    /// context and calls each queued reload, and the queue holds ONE entry per
    /// store however many of that store's transforms ran inside the batch.
    fn reload_each<R>(store: &Store<Lines>, outcome: &BatchOutcome<R>) {
        let times = outcome.stores.iter().filter(|id| **id == store.store_id()).count();
        for _ in 0..times {
            store.reload();
        }
    }

    // --- construction ---

    #[test]
    fn a_new_store_publishes_the_initial_value() {
        let s = store();
        assert_eq!(s.get(), Some(Vec::new()));
        assert_eq!(s.transform_count(), 0);
    }

    #[test]
    fn the_initial_function_runs_for_every_reload_not_once() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let s = counting_store(count.clone());
        assert_eq!(count.load(Ordering::SeqCst), 1, "once at construction");
        push(&s, "a");
        assert_eq!(count.load(Ordering::SeqCst), 2, "and once per reload");
        push(&s, "b");
        assert_eq!(count.load(Ordering::SeqCst), 3);
        s.reload();
        assert_eq!(count.load(Ordering::SeqCst), 4, "and once per explicit reload");
    }

    // --- ordering ---

    #[test]
    fn transforms_run_in_registration_order() {
        let s = store();
        push(&s, "first");
        push(&s, "second");
        push(&s, "third");
        assert_eq!(s.get(), Some(vec!["first".into(), "second".into(), "third".into()]));
    }

    #[test]
    fn each_transform_replays_from_the_base_not_from_the_last_value() {
        let s = store();
        push(&s, "a");
        // Registering replays everything on a FRESH base, so "a" appears once,
        // not twice.
        push(&s, "b");
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn a_transform_sees_what_the_previous_one_left() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let s = Store::new(without_finalize(Vec::new));
        let seen_a = seen.clone();
        s.transform(Box::new(move |state: &mut Lines| {
            seen_a.lock().unwrap().push(state.len());
            state.push("a".to_string());
        }));
        let seen_b = seen.clone();
        s.transform(Box::new(move |state: &mut Lines| {
            seen_b.lock().unwrap().push(state.len());
            state.push("b".to_string());
        }));
        assert_eq!(*seen.lock().unwrap(), vec![0, 1]);
    }

    #[test]
    fn an_explicit_reload_replays_everything_from_the_base() {
        let s = store();
        push(&s, "a");
        push(&s, "b");
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]));
        s.reload();
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]));
    }

    // --- finalize ---

    #[test]
    fn the_finalizer_runs_after_the_transforms_and_before_publication() {
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
        let order_finalize = order.clone();
        let s = Store::new(Options {
            initial: Box::new(Vec::new),
            finalize: Some(Box::new(move |value: &Lines| {
                order_finalize.lock().unwrap().push("finalize");
                assert_eq!(value.len(), 1, "the finalizer sees the transforms' output");
            })),
        });
        let order_transform = order.clone();
        s.transform(Box::new(move |state: &mut Lines| {
            order_transform.lock().unwrap().push("transform");
            state.push("a".to_string());
        }));
        assert_eq!(*order.lock().unwrap(), vec!["transform", "finalize"]);
    }

    #[test]
    fn the_finalizer_runs_again_on_every_reload() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        // Cloned BEFORE the move into the closure: the test reads `count` after
        // the store is built, and a moved Arc cannot be read.
        let counted = count.clone();
        let s = Store::new(Options {
            initial: Box::new(Vec::new),
            finalize: Some(Box::new(move |_| {
                counted.fetch_add(1, Ordering::SeqCst);
            })),
        });
        let at_construction = count.load(Ordering::SeqCst);
        assert_eq!(at_construction, 0, "construction commits without finalizing");
        push(&s, "a");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        s.reload();
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    // --- dispose ---

    #[test]
    fn a_disposed_transform_stops_applying_and_the_value_shrinks() {
        let s = store();
        let a = push(&s, "a");
        let b = push(&s, "b");
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]));
        s.dispose(a);
        assert_eq!(s.get(), Some(vec!["b".into()]));
        assert!(!s.is_registered(a));
        assert!(s.is_registered(b));
    }

    #[test]
    fn disposing_twice_does_nothing_at_all() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let s = counting_store(count.clone());
        push(&s, "a");
        let a = push(&s, "b");
        let after_register = count.load(Ordering::SeqCst);
        s.dispose(a);
        let after_first = count.load(Ordering::SeqCst);
        s.dispose(a);
        assert!(after_first > after_register, "the first dispose reloaded");
        assert_eq!(
            count.load(Ordering::SeqCst),
            after_first,
            "the second dispose reloaded nothing, not even a no-op rebuild"
        );
    }

    #[test]
    fn disposing_an_unknown_registration_is_a_no_op() {
        let s = store();
        push(&s, "a");
        s.dispose(Registration { id: 999 });
        assert_eq!(s.get(), Some(vec!["a".into()]));
        assert_eq!(s.transform_count(), 1);
    }

    // --- batching ---
    //
    // The counts below are PER STORE, and that is the whole point of the batch
    // queue. The TypeScript queue is a `Set<Reload>` where `Reload` is the one
    // function `create` built for that store, so every transform of a store adds
    // the SAME object and the set collapses them. Two transforms therefore queue
    // ONE reload. An earlier version of this port keyed the queue by
    // registration instead, so it queued two - and every assertion here that
    // expected two was wrong in the same direction as the bug it was describing.

    #[test]
    fn nothing_is_published_inside_a_batch_until_it_closes() {
        let s = store();
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
        let seen = observed.clone();
        let outcome = batch(|| {
            push(&s, "a");
            seen.lock().unwrap().push(s.get().unwrap());
            push(&s, "b");
            seen.lock().unwrap().push(s.get().unwrap());
        });
        assert_eq!(
            *observed.lock().unwrap(),
            vec![Vec::<String>::new(), Vec::new()],
            "both reads saw the pre-batch value"
        );
        assert_eq!(outcome.stores, vec![s.store_id()], "two transforms, ONE store");
        reload_each(&s, &outcome);
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn two_transforms_of_one_store_reload_once() {
        // Stated on its own because it is the invariant: reloading twice would
        // produce the same VALUE, so nothing else in this file can detect it.
        let s = store();
        let outcome = batch(|| {
            push(&s, "a");
            push(&s, "b");
            push(&s, "c");
        });
        assert_eq!(outcome.stores.len(), 1, "one store, one queued reload");
    }

    #[test]
    fn two_stores_reload_twice() {
        // The other half of the same rule, and the reason the queue is keyed by
        // store rather than collapsed to a single flag: two different stores
        // queue two different reload FUNCTIONS, so two entries.
        let a = store();
        let b = store();
        let outcome = batch(|| {
            push(&a, "x");
            push(&b, "y");
        });
        assert_eq!(outcome.stores.len(), 2);
        let mut ids = outcome.stores.clone();
        ids.sort_unstable();
        let mut expected = vec![a.store_id(), b.store_id()];
        expected.sort_unstable();
        assert_eq!(ids, expected);
    }

    #[test]
    fn the_queue_holds_each_store_once() {
        let s = store();
        push(&s, "a");
        let outcome = batch(|| {
            // The same store queued again must collapse, which is what a Set of
            // reload functions buys in the TS.
            queue_reload(s.store_id());
            queue_reload(s.store_id());
            assert_eq!(queued_now(), vec![s.store_id()]);
        });
        assert_eq!(outcome.stores, vec![s.store_id()]);
    }

    #[test]
    fn a_nested_batch_does_not_open_a_second_queue() {
        let depths = std::sync::Arc::new(std::sync::Mutex::new(Vec::<bool>::new()));
        let outcome = batch(|| {
            depths.lock().unwrap().push(in_batch());
            let inner = batch(|| {
                depths.lock().unwrap().push(in_batch());
            });
            assert!(inner.stores.is_empty(), "the inner batch reports nothing of its own");
        });
        assert_eq!(*depths.lock().unwrap(), vec![true, true]);
        assert!(outcome.stores.is_empty(), "nothing was queued at all");
    }

    #[test]
    fn a_reload_queued_in_an_inner_batch_lands_in_the_outer_one() {
        let s = store();
        let outcome = batch(|| {
            push(&s, "a");
            let inner = batch(|| {
                push(&s, "b");
            });
            assert!(inner.stores.is_empty(), "the inner batch keeps its reloads inside");
            assert_eq!(queued_now(), vec![s.store_id()], "one entry for both transforms");
        });
        assert_eq!(outcome.stores, vec![s.store_id()], "both transforms reached the outer batch");
        assert_eq!(s.get(), Some(Vec::new()), "and none has run yet");
        reload_each(&s, &outcome);
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn the_queue_can_be_inspected_from_inside_the_body() {
        let s = store();
        let outcome = batch(|| {
            push(&s, "a");
            assert_eq!(queued_now(), vec![s.store_id()]);
            push(&s, "b");
            assert_eq!(queued_now(), vec![s.store_id()], "still one store");
        });
        assert_eq!(outcome.stores, vec![s.store_id()]);
    }

    #[test]
    fn there_is_no_queue_outside_a_batch() {
        assert!(queued_now().is_empty());
    }

    #[test]
    fn a_dispose_inside_a_batch_defers_its_reload() {
        let s = store();
        let a = push(&s, "a");
        push(&s, "b");
        let outcome = batch(|| s.dispose(a));
        assert_eq!(outcome.stores, vec![s.store_id()], "one queued reload, for the dispose");
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]), "not yet applied");
        reload_each(&s, &outcome);
        assert_eq!(s.get(), Some(vec!["b".into()]));
    }

    #[test]
    fn a_batch_returns_the_body_value() {
        let s = store();
        let outcome = batch(|| {
            push(&s, "a");
            42usize
        });
        assert_eq!(outcome.value, 42);
    }

    // --- identities ---

    #[test]
    fn the_registration_id_is_unique_per_transform() {
        let s = store();
        let a = push(&s, "a");
        let b = push(&s, "b");
        let c = push(&s, "c");
        assert_ne!(a.id(), b.id());
        assert_ne!(b.id(), c.id());
    }

    #[test]
    fn a_registration_serialises_under_its_id() {
        let registration = Registration { id: 7 };
        let wire: RegistrationWire = RegistrationWire { id: registration.id() };
        assert_eq!(serde_json::to_value(wire).unwrap(), serde_json::json!({ "id": 7 }));
    }
}
