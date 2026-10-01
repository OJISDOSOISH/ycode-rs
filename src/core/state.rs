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
//!   reload and the batch runs each queued reload once at the end. Since the
//!   queue is a `Set` of reload functions and the same function is added once
//!   per registration, N transforms in one batch produce ONE reload, not N.
//!   That is the entire value of batching: a plugin group that touches six
//!   settings reloads the value once.
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

fn queue_reload(id: usize) {
    BATCHED.with(|batched| {
        if let Some(current) = batched.borrow_mut().last_mut() {
            current.insert(id);
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
        }
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
            queue_reload(id);
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
            queue_reload(registration.id);
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

/// What a batch returns: the body's value, and the reloads it queued.
///
/// The TS runs the queued reloads itself on the way out, because it holds the
/// store through the Effect context. Here the queue holds registration ids
/// rather than closures over a store, so the caller runs them - which is more
/// code but cannot quietly do the wrong thing for the wrong store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOutcome<R> {
    pub value: R,
    /// Each registration whose reload was queued, deduplicated, ascending.
    pub reloads: Vec<usize>,
}

/// `batch`: run a body, collecting the reloads it queued.
///
/// Nested calls do not open a second queue: the inner one runs its body and
/// reports nothing, so reloads queued inside it land in the outer batch and are
/// reported by the outer call.
pub fn batch<R>(body: impl FnOnce() -> R) -> BatchOutcome<R> {
    if in_batch() {
        return BatchOutcome { value: body(), reloads: Vec::new() };
    }
    BATCH_DEPTH.with(|depth| *depth.borrow_mut() += 1);
    BATCHED.with(|batched| batched.borrow_mut().push(HashSet::new()));
    let value = body();
    let queued = BATCHED.with(|batched| batched.borrow_mut().pop().unwrap_or_default());
    BATCH_DEPTH.with(|depth| *depth.borrow_mut() -= 1);
    let mut reloads: Vec<usize> = queued.into_iter().collect();
    reloads.sort_unstable();
    BatchOutcome { value, reloads }
}

/// The reloads queued by the innermost open batch, ascending.
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
    fn counting_store(count: &std::sync::atomic::AtomicUsize) -> Store<Lines> {
        Store::new(without_finalize(move || {
            count.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        }))
    }

    fn push(store: &Store<Lines>, value: &str) -> Registration {
        store.transform(Box::new(move |state: &mut Lines| state.push(value.to_string())))
    }

    /// Applies the reloads a batch queued, which is what `batch` does on the way
    /// out in the TS.
    fn apply(store: &Store<Lines>, reloads: &[usize]) {
        for _ in reloads {
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
        let count = std::sync::atomic::AtomicUsize::new(0);
        let s = counting_store(&count);
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
        let count = std::sync::atomic::AtomicUsize::new(0);
        let s = Store::new(Options {
            initial: Box::new(Vec::new),
            finalize: Some(Box::new(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
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
        let count = std::sync::atomic::AtomicUsize::new(0);
        let s = counting_store(&count);
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

    #[test]
    fn inside_a_batch_nothing_is_published_until_the_batch_closes() {
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
        assert_eq!(outcome.reloads.len(), 2);
        apply(&s, &outcome.reloads);
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn a_batch_with_no_transform_queues_nothing() {
        let s = store();
        let outcome = batch(|| push(&s, "a"));
        // `push` reloads immediately when no batch is open, and this one is open.
        assert_eq!(outcome.reloads.len(), 1);
        assert_eq!(s.get(), Some(Vec::new()));
        apply(&s, &outcome.reloads);
        assert_eq!(s.get(), Some(vec!["a".into()]));
    }

    #[test]
    fn the_queue_holds_each_registration_once() {
        let s = store();
        let a = push(&s, "a");
        let outcome = batch(|| {
            // The same registration queued twice must collapse, which is what a
            // Set of reload functions buys in the TS.
            queue_reload(a.id());
            queue_reload(a.id());
        });
        assert_eq!(outcome.reloads, vec![a.id()]);
    }

    #[test]
    fn a_nested_batch_does_not_open_a_second_queue() {
        let depths = std::sync::Arc::new(std::sync::Mutex::new(Vec::<bool>::new()));
        let outcome = batch(|| {
            depths.lock().unwrap().push(in_batch());
            let inner = batch(|| {
                depths.lock().unwrap().push(in_batch());
            });
            assert!(inner.reloads.is_empty(), "the inner batch reports nothing of its own");
        });
        assert_eq!(*depths.lock().unwrap(), vec![true, true]);
        assert!(outcome.reloads.is_empty(), "nothing was queued at all");
    }

    #[test]
    fn a_reload_queued_in_an_inner_batch_lands_in_the_outer_one() {
        let s = store();
        let outcome = batch(|| {
            push(&s, "a");
            let inner = batch(|| {
                push(&s, "b");
            });
            assert!(inner.reloads.is_empty(), "the inner batch keeps its reloads inside");
        });
        assert_eq!(outcome.reloads.len(), 2, "both registrations reached the outer batch");
        assert_eq!(s.get(), Some(Vec::new()), "and neither has run yet");
        apply(&s, &outcome.reloads);
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn the_queue_can_be_inspected_from_inside_the_body() {
        let s = store();
        let outcome = batch(|| {
            push(&s, "a");
            assert_eq!(queued_now().len(), 1);
            push(&s, "b");
            assert_eq!(queued_now().len(), 2);
        });
        assert_eq!(outcome.reloads.len(), 2);
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
        assert_eq!(outcome.reloads, vec![a.id()], "one queued reload, for the dispose");
        assert_eq!(s.get(), Some(vec!["a".into(), "b".into()]), "not yet applied");
        apply(&s, &outcome.reloads);
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
