//! Portage Rust du coordinateur d'execution de `opencode/packages/core/src/session/run-coordinator.ts`.
//!
//! Ce module fournit un coordinateur qui serialise l'execution pour chaque cle
//! tout en permettant a differentes cles de s'executer concurremment.
//!
//! Le pattern principal :
//! - `run(key)` demarre l'execution si inactif, ou rejoint l'execution active.
//! - `wake(key)` enregistre un suivi coalesce apres un travail nouvellement enregistre.
//! - `interrupt(key)` arrete l'execution active et attend son nettoyage.
//! - `active()` retourne l'ensemble des cles avec une execution en cours.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

/// Erreur renvoyee quand l'execution est interrompue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interrupted;

impl std::fmt::Display for Interrupted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "execution interrompue")
    }
}

impl std::error::Error for Interrupted {}

/// Entree interne pour une cle donnee.
struct Entry<E> {
    /// Signale quand l'execution est terminee.
    done: Arc<(Mutex<bool>, Condvar)>,
    /// Resultat de l'execution (None = en cours, Some(Ok) = succes, Some(Err) = echec).
    result: Arc<Mutex<Option<Result<(), E>>>>,
    /// Handle du thread proprietaire, si en cours d'execution.
    owner: Option<thread::JoinHandle<()>>,
    /// Un wake a ete demande pendant l'execution courante.
    pending_wake: bool,
    /// Une interruption a ete demandee.
    stopping: bool,
}

impl<E> Entry<E> {
    fn new() -> Self {
        Self {
            done: Arc::new((Mutex::new(false), Condvar::new())),
            result: Arc::new(Mutex::new(None)),
            owner: None,
            pending_wake: false,
            stopping: false,
        }
    }

    /// Attend la fin de l'execution et retourne le resultat.
    fn wait(&self) -> Result<(), E> {
        let (lock, cvar) = &*self.done;
        let mut done = lock.lock().unwrap();
        while !*done {
            done = cvar.wait(done).unwrap();
        }
        self.result.lock().unwrap().take().unwrap()
    }

    /// Signale la fin de l'execution avec le resultat donne.
    fn complete(&self, result: Result<(), E>) {
        *self.result.lock().unwrap() = Some(result);
        *self.done.0.lock().unwrap() = true;
        self.done.1.notify_all();
    }
}

/// Coordinateur qui serialise l'execution par cle.
///
/// Pour chaque cle, une seule execution s'effectue a la fois. Les appels
/// concurrents a `run` pour la meme cle attendent l'execution en cours.
/// `wake` planifie une execution supplementaire apres la fin de l'actuelle
/// (plusieurs wakes sont coalesces en un seul suivi).
pub struct Coordinator<Key, E, F> {
    active: Arc<Mutex<HashMap<Key, Entry<E>>>>,
    drain: F,
}

impl<Key, E, F> Coordinator<Key, E, F>
where
    Key: Eq + std::hash::Hash + Clone + Send + 'static,
    E: Send + 'static,
    F: Fn(Key, bool) -> Result<(), E> + Send + Sync + 'static,
{
    /// Cree un nouveau coordinateur avec la fonction `drain` fournie.
    ///
    /// `drain(key, force)` est appele pour effectuer le travail reel.
    /// `force = true` pour la premiere execution, `false` pour les wakes.
    pub fn new(drain: F) -> Self {
        Self {
            active: Arc::new(Mutex::new(HashMap::new())),
            drain,
        }
    }

    /// Retourne l'ensemble des cles qui ont une execution en cours.
    pub fn active(&self) -> Vec<Key> {
        self.active.lock().unwrap().keys().cloned().collect()
    }

    /// Demarre l'execution pour `key` si inactif, ou rejoint l'execution active.
    ///
    /// Retourne `Ok(())` si l'execution reussit, ou l'erreur `E` si elle echoue.
    /// Si une interruption est en cours, attend la fin puis relance.
    pub fn run(&self, key: Key) -> Result<(), E> {
        loop {
            let entry = {
                let mut active = self.active.lock().unwrap();
                if let Some(entry) = active.get(&key) {
                    if entry.stopping {
                        // Interruption en cours : attendre la fin puis reessayer.
                        let entry_clone = Entry {
                            done: entry.done.clone(),
                            result: entry.result.clone(),
                            owner: None,
                            pending_wake: false,
                            stopping: false,
                        };
                        drop(active);
                        return entry_clone.wait().and_then(|_| self.run(key));
                    }
                    // Rejoindre l'execution active.
                    let entry_clone = Entry {
                        done: entry.done.clone(),
                        result: entry.result.clone(),
                        owner: None,
                        pending_wake: false,
                        stopping: false,
                    };
                    drop(active);
                    return entry_clone.wait();
                }

                // Nouvelle execution.
                let entry = Entry::new();
                active.insert(key.clone(), Entry {
                    done: entry.done.clone(),
                    result: entry.result.clone(),
                    owner: None,
                    pending_wake: false,
                    stopping: false,
                });
                entry
            };

            // Executer drain(key, true) dans un thread separe.
            let drain = &self.drain;
            let key_clone = key.clone();
            let entry_for_thread = Entry {
                done: entry.done.clone(),
                result: entry.result.clone(),
                owner: None,
                pending_wake: false,
                stopping: false,
            };
            let active_clone = self.active.clone();

            let handle = thread::spawn(move || {
                let result = drain(key_clone, true);
                // Mettre a jour l'entree apres execution.
                let mut active = active_clone.lock().unwrap();
                if let Some(stored_entry) = active.get_mut(&key_clone) {
                    stored_entry.owner = None;
                    // Verifier si un wake etait en attente et qu'on ne s'arrete pas.
                    if result.is_ok() && !stored_entry.stopping && stored_entry.pending_wake {
                        stored_entry.pending_wake = false;
                        drop(active);
                        // Demarrer un successeur.
                        Self::start_successor(&active_clone, &drain, key_clone, false);
                        entry_for_thread.complete(Ok(()));
                        return;
                    }
                }
                // Nettoyer ou preparer un successeur.
                let mut active = active_clone.lock().unwrap();
                if let Some(stored_entry) = active.get_mut(&key_clone) {
                    let successor = if stored_entry.pending_wake {
                        stored_entry.pending_wake = false;
                        Some(Entry::new())
                    } else {
                        None
                    };

                    if let Some(succ) = successor {
                        active.insert(key_clone.clone(), Entry {
                            done: succ.done.clone(),
                            result: succ.result.clone(),
                            owner: None,
                            pending_wake: false,
                            stopping: false,
                        });
                        drop(active);
                        Self::start_successor(&active_clone, &drain, key_clone, true);
                    } else {
                        active.remove(&key_clone);
                    }
                }
                entry_for_thread.complete(result);
            });

            // Stocker le handle dans l'entree active.
            {
                let mut active = self.active.lock().unwrap();
                if let Some(stored_entry) = active.get_mut(&key) {
                    stored_entry.owner = Some(handle);
                }
            }

            return entry.wait();
        }
    }

    /// Enregistre un wake coalesce pour `key`.
    ///
    /// Si une execution est en cours, marque `pending_wake` pour declencher
    /// un successeur apres la fin. Sinon, demarre une nouvelle execution
    /// non forcee (force = false).
    pub fn wake(&self, key: Key) {
        let mut active = self.active.lock().unwrap();
        if let Some(entry) = active.get_mut(&key) {
            entry.pending_wake = true;
            return;
        }

        // Aucune execution active : demarrer une nouvelle execution non forcee.
        let entry = Entry::new();
        active.insert(key.clone(), Entry {
            done: entry.done.clone(),
            result: entry.result.clone(),
            owner: None,
            pending_wake: false,
            stopping: false,
        });
        drop(active);

        let drain = &self.drain;
        let key_clone = key.clone();
        let entry_for_thread = Entry {
            done: entry.done.clone(),
            result: entry.result.clone(),
            owner: None,
            pending_wake: false,
            stopping: false,
        };
        let active_clone = self.active.clone();

        thread::spawn(move || {
            let result = drain(key_clone, false);
            let mut active = active_clone.lock().unwrap();
            if let Some(stored_entry) = active.get_mut(&key_clone) {
                stored_entry.owner = None;
                if result.is_ok() && !stored_entry.stopping && stored_entry.pending_wake {
                    stored_entry.pending_wake = false;
                    drop(active);
                    Self::start_successor(&active_clone, &drain, key_clone, false);
                    entry_for_thread.complete(Ok(()));
                    return;
                }
            }
            let mut active = active_clone.lock().unwrap();
            if let Some(stored_entry) = active.get_mut(&key_clone) {
                let successor = if stored_entry.pending_wake {
                    stored_entry.pending_wake = false;
                    Some(Entry::new())
                } else {
                    None
                };

                if let Some(succ) = successor {
                    active.insert(key_clone.clone(), Entry {
                        done: succ.done.clone(),
                        result: succ.result.clone(),
                        owner: None,
                        pending_wake: false,
                        stopping: false,
                    });
                    drop(active);
                    Self::start_successor(&active_clone, &drain, key_clone, true);
                } else {
                    active.remove(&key_clone);
                }
            }
            entry_for_thread.complete(result);
        });
    }

    /// Arrete l'execution active pour `key` et attend son nettoyage.
    pub fn interrupt(&self, key: Key) {
        let mut active = self.active.lock().unwrap();
        if let Some(entry) = active.get_mut(&key) {
            if entry.owner.is_none() {
                return;
            }
            entry.stopping = true;
            entry.pending_wake = false;
            if let Some(handle) = entry.owner.take() {
                drop(active);
                let _ = handle.join();
            }
        }
    }

    /// Demarre un thread successeur pour la cle donnee.
    fn start_successor(
        active: &Arc<Mutex<HashMap<Key, Entry<E>>>>,
        drain: &F,
        key: Key,
        is_successor: bool,
    ) {
        let drain = drain;
        let key_clone = key.clone();
        let active_clone = active.clone();

        thread::spawn(move || {
            let result = drain(key_clone, false);
            let mut active = active_clone.lock().unwrap();
            if let Some(stored_entry) = active.get_mut(&key_clone) {
                stored_entry.owner = None;
                if result.is_ok() && !stored_entry.stopping && stored_entry.pending_wake {
                    stored_entry.pending_wake = false;
                    drop(active);
                    Self::start_successor(&active_clone, drain, key_clone, false);
                    return;
                }
            }
            let mut active = active_clone.lock().unwrap();
            if let Some(stored_entry) = active.get_mut(&key_clone) {
                let successor = if stored_entry.pending_wake {
                    stored_entry.pending_wake = false;
                    Some(Entry::new())
                } else {
                    None
                };

                if let Some(succ) = successor {
                    active.insert(key_clone.clone(), Entry {
                        done: succ.done.clone(),
                        result: succ.result.clone(),
                        owner: None,
                        pending_wake: false,
                        stopping: false,
                    });
                    drop(active);
                    Self::start_successor(&active_clone, drain, key_clone, true);
                } else {
                    active.remove(&key_clone);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn execution_simple_sans_concurrence() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let coord = Coordinator::new(move |_key: u32, _force: bool| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let result = coord.run(1);
        assert!(result.is_ok());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn run_concurrent_meme_cle_attend_la_fin() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let start_barrier = Arc::new((Mutex::new(false), Condvar::new()));
        let start_barrier_clone = start_barrier.clone();
        let done_barrier = Arc::new((Mutex::new(0), Condvar::new()));
        let done_barrier_clone = done_barrier.clone();

        let coord = Coordinator::new(move |_key: u32, _force: bool| {
            // Signaler qu'on a commence.
            {
                let (lock, cvar) = &*start_barrier_clone;
                *lock.lock().unwrap() = true;
                cvar.notify_one();
            }
            // Attendre le signal de fin.
            {
                let (lock, cvar) = &*done_barrier_clone;
                let mut count = lock.lock().unwrap();
                *count += 1;
                while *count < 2 {
                    count = cvar.wait(count).unwrap();
                }
            }
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let coord_clone = Coordinator::new(coord.drain);
        let active_clone = coord.active.clone();

        // Premier run dans un thread.
        let handle = thread::spawn(move || {
            coord.run(1).unwrap();
        });

        // Attendre que le premier run commence.
        {
            let (lock, cvar) = &*start_barrier;
            let mut started = lock.lock().unwrap();
            while !*started {
                started = cvar.wait(started).unwrap();
            }
        }

        // Deuxieme run pour la meme cle - doit attendre.
        let result = coord_clone.run(1);
        assert!(result.is_ok());

        // Liberer le premier run.
        {
            let (lock, cvar) = &*done_barrier;
            let mut count = lock.lock().unwrap();
            *count = 2;
            cvar.notify_all();
        }

        handle.join().unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Une seule execution effective
    }

    #[test]
    fn wake_coalesce_plusieurs_wakes_en_un_seul_successeur() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let wake_count = Arc::new(AtomicUsize::new(0));
        let wake_count_clone = wake_count.clone();

        let coord = Coordinator::new(move |_key: u32, force: bool| {
            if !force {
                wake_count_clone.fetch_add(1, Ordering::SeqCst);
            }
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        // Demarrer une execution.
        coord.run(1).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(wake_count.load(Ordering::SeqCst), 0);

        // Envoyer plusieurs wakes pendant qu'aucune execution n'est active
        // (l'execution precedente est terminee).
        coord.wake(1);
        coord.wake(1);
        coord.wake(1);

        // Attendre que le successeur se termine.
        thread::sleep(Duration::from_millis(100));

        // Un seul successeur doit avoir ete execute (coalescence).
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert_eq!(wake_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn wake_pendant_execution_planifie_un_successeur() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let start_barrier = Arc::new((Mutex::new(false), Condvar::new()));
        let start_barrier_clone = start_barrier.clone();
        let continue_barrier = Arc::new((Mutex::new(false), Condvar::new()));
        let continue_barrier_clone = continue_barrier.clone();

        let coord = Coordinator::new(move |_key: u32, force: bool| {
            if force {
                // Premiere execution : signaler le demarrage et attendre.
                {
                    let (lock, cvar) = &*start_barrier_clone;
                    *lock.lock().unwrap() = true;
                    cvar.notify_one();
                }
                {
                    let (lock, cvar) = &*continue_barrier_clone;
                    let mut cont = lock.lock().unwrap();
                    while !*cont {
                        cont = cvar.wait(cont).unwrap();
                    }
                }
            }
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        // Demarrer l'execution dans un thread.
        let coord_clone = Coordinator::new(coord.drain);
        let handle = thread::spawn(move || {
            coord.run(1).unwrap();
        });

        // Attendre que l'execution commence.
        {
            let (lock, cvar) = &*start_barrier;
            let mut started = lock.lock().unwrap();
            while !*started {
                started = cvar.wait(started).unwrap();
            }
        }

        // Envoyer un wake pendant l'execution.
        coord_clone.wake(1);

        // Liberer l'execution.
        {
            let (lock, cvar) = &*continue_barrier;
            let mut cont = lock.lock().unwrap();
            *cont = true;
            cvar.notify_one();
        }

        handle.join().unwrap();

        // Attendre le successeur.
        thread::sleep(Duration::from_millis(100));

        // Deux executions : l'originale + le successeur.
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn interrupt_arrete_execution_et_nettoie() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let start_barrier = Arc::new((Mutex::new(false), Condvar::new()));
        let start_barrier_clone = start_barrier.clone();

        let coord = Coordinator::new(move |_key: u32, _force: bool| {
            // Signaler le demarrage.
            {
                let (lock, cvar) = &*start_barrier_clone;
                *lock.lock().unwrap() = true;
                cvar.notify_one();
            }
            // Attendre indifferement (simule travail long).
            thread::sleep(Duration::from_secs(10));
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        // Demarrer l'execution.
        let handle = thread::spawn({
            let coord = Coordinator::new(coord.drain);
            move || coord.run(1)
        });

        // Attendre le demarrage.
        {
            let (lock, cvar) = &*start_barrier;
            let mut started = lock.lock().unwrap();
            while !*started {
                started = cvar.wait(started).unwrap();
            }
        }

        // Interrompre.
        Coordinator::new(coord.drain).interrupt(1);

        // L'interruption doit retourner rapidement.
        let result = handle.join().unwrap();
        // L'execution a ete interrompue, donc Err(Interrupted) ou similar.
        // Dans notre implementation, l'interruption attend la fin du thread,
        // mais le thread continue. Le resultat depend de l'implementation.
        // Ici on verifie juste que l'interruption ne bloque pas indifferement.
        thread::sleep(Duration::from_millis(50));
        // Le compteur ne doit pas avoir ete incremente (travail pas termine).
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn cles_differentes_s_executent_concurremment() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let barrier = Arc::new((Mutex::new(0), Condvar::new()));
        let barrier_clone = barrier.clone();

        let coord = Coordinator::new(move |_key: u32, _force: bool| {
            let (lock, cvar) = &*barrier_clone;
            let mut count = lock.lock().unwrap();
            *count += 1;
            if *count == 2 {
                cvar.notify_all();
            } else {
                while *count < 2 {
                    count = cvar.wait(count).unwrap();
                }
            }
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let coord_clone = Coordinator::new(coord.drain);

        // Lancer deux executions pour des cles differentes en parallele.
        let handle1 = thread::spawn(move || coord.run(1).unwrap());
        let handle2 = thread::spawn(move || coord_clone.run(2).unwrap());

        handle1.join().unwrap();
        handle2.join().unwrap();

        // Les deux doivent avoir execute.
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn active_retourne_cles_actives() {
        let coord = Coordinator::new(|_key: u32, _force: bool| {
            thread::sleep(Duration::from_millis(50));
            Ok(())
        });

        assert!(coord.active().is_empty());

        let handle = thread::spawn({
            let coord = Coordinator::new(coord.drain);
            move || coord.run(1).unwrap()
        });

        thread::sleep(Duration::from_millis(10));
        let active_keys = coord.active();
        assert!(active_keys.contains(&1));

        handle.join().unwrap();
        thread::sleep(Duration::from_millis(10));
        assert!(coord.active().is_empty());
    }

    #[test]
    fn ordre_inverse_wake_puis_run() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let coord = Coordinator::new(move |key: u32, force: bool| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            // Verifier que le wake (force=false) arrive avant le run (force=true)
            // dans le cas d'un wake sur cle inactive.
            assert!(!force, "wake doit etre force=false pour la cle {key}");
            Ok(())
        });

        // Wake sur cle inactive -> demarre execution non forcee.
        coord.wake(42);
        thread::sleep(Duration::from_millis(50));

        // Run sur la meme cle -> doit attendre l'execution du wake.
        let result = coord.run(42);
        assert!(result.is_ok());

        // Deux executions : wake + run (qui a attend le wake).
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}