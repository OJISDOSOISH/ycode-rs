//! Portage de `packages/core/src/pty/pty.ts`.
//!
//! # Ce que contient la source
//!
//! Vingt-cinq lignes, quatre declarations de types, **aucune instruction** :
//!
//! ```ts
//! export type Disp = { dispose(): void }
//! export type Exit = { exitCode: number, signal?: number | string }
//! export type Opts = { name: string, cols?: number, rows?: number, cwd?: string, env?: Record<string, string> }
//! export type Proc = { pid: number, onData(...): Disp, onExit(...): Disp, write(...), resize(...), kill(signal?) }
//! ```
//!
//! En TypeScript, `type` est un alias de forme : il est efface a la compilation,
//! ne genere aucun code et n'est verifie qu'au moment ou une valeur est
//! assignee. Ce fichier est donc un **contrat** et rien d'autre. Les
//! implementations sont ailleurs, dans le meme repertoire : `pty.node.ts`, qui
//! delegue a `@lydell/node-pty`, et `pty.bun.ts`, qui delegue au module
//! equivalent de Bun.
//!
//! Ce fichier ne fabrique donc **ni processus, ni terminal, ni fichier** :
//! il n'y a rien a y demarrer. Une implementation honnete de `spawn` est
//! impossible ici, et c'est explique dans la section suivante.
//!
//! # Pourquoi aucune implementation de pseudo terminal n'est portee
//!
//! Un pseudo terminal reel n'est pas portable sans dependance systeme, et
//! `Cargo.toml` n'en declare aucune. Concretement, ce qu'il faudrait pour
//! remplacer `node-pty` :
//!
//! - sur Unix, `forkpty` ou `posix_openpt` + `grantpt` + `unlockpt` +
//!   `ptsname`, qui vivent dans une bibliotheque C et n'existent pas dans la
//!   bibliotheque standard de Rust ;
//! - sur Windows, `CreatePseudoConsole` (ConPTY) ou le pilote `winpty`, qui
//!   demandent un appel systeme Windows ;
//! - dans les deux cas, une lecture bloquante sur le descripteur, une
//!   temporisation d'E/S et la conversion des octets en texte.
//!
//! Aucun de ces Elements n'est disponible sans crate externe, et la regle de
//! portage interdit de modifier `Cargo.toml`. Ecrire une approximation avec
//! `std::process::Command` serait pire que rien : `Command` ouvre des
//! canaux, pas un terminal, donc `resize` n'aurait aucun effet et les
//! programmes interactifs refuseraient de demarrer. Ce fichier se limite donc
//! au contrat, ce qui est exactement ce que la source contient.
//!
//! # Les quatre ecarts de forme, assumes et documentes
//!
//! 1. **Type structurel contre trait.** En TypeScript, n'importe quel objet
//!    dont la forme correspond satisfait `Proc`, sans rien declarer. En Rust il
//!    faut ecrire `impl Proc`. C'est la seule facon d'avoir une methode sans
//!    implementation, donc le `trait` est la traduction honnete.
//! 2. **`pid` devient un accesseur.** La source expose une propriete ; un
//!    `trait` Rust ne porte pas de champ. Le nom reste `pid`, en methode.
//!    La valeur ne change pas pendant la vie du processus, donc aucun etat
//!    interieur n'est necessaire.
//! 3. **Les chaines sont empruntees.** `write(data: string)` devient
//!    `write(&self, data: &str)`. Les chaines JavaScript sont immuables, donc
//!    la copie faite par l'appelant n'apporte rien ; l'emprunt evite un copie
//!    de la sortie du terminal, qui peut peser plusieurs kilo-octets.
//! 4. **`number` devient un entier.** Un `number` TypeScript est un flottant
//!    double. Ici `pid`, `cols`, `rows`, `exitCode` et le `signal` numerique
//!    sont des entiers, parce que toutes les valeurs reelles le sont : un
//!    identifiant de processus tient dans 32 bits, une taille de terminal est
//!    un petit entier positif, un code de sortie et un numero de signal aussi.
//!    Le cout est connu et borne : une valeur fractionnaire venue du
//!    TypeScript echoue a la deserialisation au lieu d etre tronquee en
//!    silence.
//!
//! # Deux pieges `?` contre `??` qui n auraient pas de traduction evidente
//!
//! - `kill(signal?)` : `None` doit etre transmis comme **absent**, jamais
//!   converti en chaine vide, parce que `""` est un nom de signal invalide et
//!   que le portage qui ecrirait `unwrap_or_default()` enverrait `""`.
//! - `cols?` et `rows?` absents : ils valent **taille par defaut** du
//!   pseudo terminal, pas `0`. Un `0` demande explicitement une taille nulle.
//! - `env?` absent : l'implementation herite de l'environnement du pere. Une
//!   table vide, elle, demande un environnement sans aucune variable. Les
//!   deux cas sont distincts et le portage les garde distincts
//!   (`None` contre `Some(table vide)`).
//!
//! Ces trois valeurs absentes ne sont donc jamais remplacees par une valeur
//! neutre : c est le point que les tests de ce fichier verifient.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Disp
// ---------------------------------------------------------------------------

/// Abonnement jetable, equivalent de `export type Disp`.
///
/// La source est `{ dispose(): void }` : un objet qui ne sert qu a retirer un
/// abonnement. Il est rendu par `on_data` et par `on_exit`, qui renvoient
/// donc tous les deux `Box<dyn Disp>`.
///
/// `dispose` prend `&self` et pas `&mut self` : un abonnement est partage
/// entre l appelant et l objet qui l a cree, et il doit pouvoir etre retire
/// depuis un partage `Arc` sans prendre l objet entier.
pub trait Disp {
    /// Retire l abonnement. Appeler plusieurs fois doit rester sans effet,
    /// comme en JavaScript ou le meme objet peut etre `dispose` deux fois.
    fn dispose(&self);
}

// ---------------------------------------------------------------------------
// Exit
// ---------------------------------------------------------------------------

/// Forme du champ `signal` de `Exit`, qui est `number | string` en amont.
///
/// La source ne precise ni jeu de signaux autorises ni conversion entre les
/// deux formes : c est un union simple, donc un enum a deux variantes. La
/// representation est **sans tag** (`untagged`) pour que le JSON reste
/// exactement ce que le TypeScript produirait : `9` ou `"SIGKILL"`, jamais
/// `{ "nombre": 9 }`.
///
/// Le nom des variantes est en francais, il n apparait donc jamais dans le
/// JSON : `#[serde(untagged)]` ne fait pas intervenir de discriminant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Signal {
    /// Forme numerique, la plus portable : c est ce que produit un systeme
    /// qui n'a pas de nom de signal, typiquement Windows.
    Nombre(i64),
    /// Forme textuelle, celle des systemes Unix : `"SIGKILL"`, `"SIGTERM"`,
    /// `"SIGHUP"`. La casse et le contenu ne sont valides par aucun controle
    /// ici, la source n en impose aucun.
    Nom(String),
}

/// Evenement de fin d'un pseudo terminal, equivalent de `export type Exit`.
///
/// - `exitCode` devient `exit_code`, avec le renommage de cle explicite
///   ci-dessous, car la source est en camelCase.
/// - `signal?` devient `Option<Signal>`. Un `signal` absent ne doit jamais
///   etre remplace par une valeur neutre : ni `0`, ni `""`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exit {
    /// Code de sortie du processus fils, nom JSON exact `exitCode`.
    #[serde(rename = "exitCode")]
    pub exit_code: i64,
    /// Signal qui a tue le processus, absent si le processus a rendu la main
    /// tout seul. Nom JSON exact `signal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<Signal>,
}

// ---------------------------------------------------------------------------
// Opts
// ---------------------------------------------------------------------------

/// Options de creation d'un pseudo terminal, equivalent de
/// `export type Opts`.
///
/// Seul `name` est obligatoire. Tous les autres champs sont optionnels et
/// leur absence est une information : elle demande une valeur par defaut a
/// l'implementation, pas une valeur neutre.
///
/// Les cinq noms sont des mots uniques en minuscules, donc aucun renommage
/// camelCase n est necessaire ; le contrat de nom est aussi verifie par les
/// tests, qui serialisent la structure et relisent les cles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opts {
    /// Nom du programme a lancer. Nom JSON exact `name`.
    pub name: String,
    /// Nombre de colonnes, absent pour la taille par defaut. Nom JSON exact
    /// `cols`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u32>,
    /// Nombre de lignes, absent pour la taille par defaut. Nom JSON exact
    /// `rows`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u32>,
    /// Repertoire de travail, absent pour heriter de celui du processus
    /// courant. Nom JSON exact `cwd`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Environnement du processus fils, absent pour heriter de celui du
    /// processus courant. Nom JSON exact `env`.
    ///
    /// Une table **vide** n est pas la meme chose qu un environnement absent :
    /// la premiere demande un processus sans aucune variable, la seconde
    /// demande l heritage. Le type les distingue.
    ///
    /// `Record<string, string>` devient `BTreeMap` pour un ordre
    /// deterministe, conformement aux conventions du portage. Consequence
    /// connue : les cles sortent par ordre alphabetique, alors que
    /// JavaScript les sort dans l ordre d insertion. L'ordre d'un objet JSON
    /// n'a pas de sens, donc seule la comparaison octet pour octet de deux
    /// serialisations differerait.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
}

impl Opts {
    /// Construit les options minimales : uniquement `name`, tout le reste
    /// absent, donc demande les valeurs par defaut a l'implementation.
    ///
    /// Cette commodite n'existe pas dans la source, ou `name` est simplement
    /// un champ d'objet. Elle evite d'oublier un champ obligatoire.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cols: None,
            rows: None,
            cwd: None,
            env: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Proc
// ---------------------------------------------------------------------------

/// Ecouteur de sortie du terminal, equivalent du parametre de `onData`.
///
/// La source prend `(data: string) => void`. La forme `FnMut` autorise
/// l accumulation dans la variable capturee sans avoir a synchroniser une
/// structure exterieure, ce qui est le besoin reel d un journal de sortie.
///
/// `Send` est exige parce qu une implementation plausible de `Proc` lit le
/// terminal depuis une tache de fond, et `'static` parce que la fermeture doit
/// survivre a l appel qui l enregistre. La source n a rien de tout cela parce
/// que JavaScript n a pas de systeme de types de threads : c est une
/// adaptation au monde cible, pas un comportement ajoute.
pub type DataListener = Box<dyn FnMut(&str) + Send + 'static>;

/// Ecouteur de fin de processus, equivalent du parametre de `onExit`.
///
/// Memes conditions que [DataListener] : la source est `(event: Exit) => void`.
pub type ExitListener = Box<dyn FnMut(Exit) + Send + 'static>;

/// Processus attache a un pseudo terminal, equivalent de `export type Proc`.
///
/// Une implementation concrete est un pseudo terminal reel, donc elle n'existe
/// pas dans ce crate. Ce trait sert de contrat a une implementation de systeme
/// qui sera ecrite ailleurs, et il est utilisable comme objet : `Box<dyn Proc>`.
///
/// Toutes les methodes prennent `&self` et ne rendent que des types
/// ordinaires, donc le trait reste utilisable comme objet : `Box<dyn Proc>`
/// fonctionne, sans genericite ni bornes a ajouter.
pub trait Proc {
    /// Identifiant du processus fils. La source expose un champ `pid` ; un
    /// `trait` ne portant pas de champ, cela devient un accesseur qui porte le
    /// meme nom. La valeur ne change pas pendant la vie du processus.
    fn pid(&self) -> u32;

    /// Abonne un ecouteur aux morceaux de sortie du terminal, comme
    /// `onData(listener)` dans la source.
    ///
    /// La source passe des chaines, donc le contrat recoit `&str`. Attention
    /// pour une future implementation branchee sur des octets bruts : un
    /// caractere multi octets peut etre coupe en deux entre deux morceaux.
    /// La source ne dit rien de ce decoupage, donc il n est ni impose ni
    /// tranche ici ; il appartient a l'implementateur.
    ///
    /// Le retour est l abonnement jetable, comme dans la source.
    fn on_data(&self, listener: DataListener) -> Box<dyn Disp>;

    /// Abonne un ecouteur a la fin du processus, comme `onExit(listener)`.
    /// Le retour est l abonnement jetable, comme dans la source.
    fn on_exit(&self, listener: ExitListener) -> Box<dyn Disp>;

    /// Ecrit des donnees dans l entree du terminal, comme `write(data)`.
    fn write(&self, data: &str);

    /// Redimensionne le terminal, comme `resize(cols, rows)`. Les deux
    /// dimensions sont obligatoires en amont comme ici.
    fn resize(&self, cols: u32, rows: u32);

    /// Tue le processus, comme `kill(signal?)`.
    ///
    /// `None` doit etre transmis comme **absent**, pour que l'implementation
    /// applique son signal par defaut. Il ne faut surtout pas le remplacer par
    /// une chaine vide : `""` est un nom de signal invalide, et c est
    /// exactement le piege `?` contre `??` du projet.
    fn kill(&self, signal: Option<&str>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard};

    // -- doubles d essai ----------------------------------------------------
    //
    // Ces doubles ne simulent ni terminal ni processus fils : ils notent les
    // appels recus et distribuent les ecouteurs immediatement, parce qu il n y
    // a rien derriere. Ils servent uniquement a prouver que le contrat est
    // implementable et utilisable comme objet.

    /// Verrouille sans paniquer si un autre test a entre en panique, pour ne
    /// pas transformer un echec de test en echec de tous les autres.
    fn verrouiller<T>(verrou: &Mutex<T>) -> MutexGuard<'_, T> {
        verrou.lock().unwrap_or_else(|e| e.into_inner())
    }

    struct DispFaux {
        abonne: Arc<AtomicBool>,
    }

    impl Disp for DispFaux {
        fn dispose(&self) {
            self.abonne.store(false, Ordering::SeqCst);
        }
    }

    #[derive(Clone)]
    struct ProcFaux {
        pid: u32,
        journal: Arc<Mutex<Vec<String>>>,
        abonne: Arc<AtomicBool>,
        sortie: Exit,
    }

    impl ProcFaux {
        fn nouveau() -> Self {
            Self {
                pid: 4242,
                journal: Arc::new(Mutex::new(Vec::new())),
                abonne: Arc::new(AtomicBool::new(true)),
                sortie: Exit {
                    exit_code: 137,
                    signal: Some(Signal::Nombre(9)),
                },
            }
        }
    }

    impl Proc for ProcFaux {
        fn pid(&self) -> u32 {
            self.pid
        }

        fn on_data(&self, mut listener: DataListener) -> Box<dyn Disp> {
            listener("premier morceau");
            listener("second morceau");
            Box::new(DispFaux {
                abonne: Arc::clone(&self.abonne),
            })
        }

        fn on_exit(&self, mut listener: ExitListener) -> Box<dyn Disp> {
            listener(self.sortie.clone());
            Box::new(DispFaux {
                abonne: Arc::clone(&self.abonne),
            })
        }

        fn write(&self, data: &str) {
            verrouiller(&self.journal).push(format!("write({})", data));
        }

        fn resize(&self, cols: u32, rows: u32) {
            verrouiller(&self.journal).push(format!("resize({},{})", cols, rows));
        }

        fn kill(&self, signal: Option<&str>) {
            // L'absence et la chaine vide doivent rester deux appels
            // distinguables dans le journal, donc le marqueur est explicite.
            let entree = match signal {
                None => "kill(aucun)".to_string(),
                Some(nom) => format!("kill({})", nom),
            };
            verrouiller(&self.journal).push(entree);
        }
    }

    // -- Opts ---------------------------------------------------------------

    #[test]
    fn des_options_avec_le_seul_champ_obligatoire_ne_donnent_que_le_nom() {
        let opts = Opts::new("bash");
        let json = serde_json::to_string(&opts).expect("serialisation");
        assert_eq!(json, r#"{"name":"bash"}"#);
    }

    #[test]
    fn des_options_lues_depuis_le_json_ont_les_champs_optionnels_absents() {
        let opts: Opts = serde_json::from_str(r#"{"name":"pwsh"}"#).expect("deserialisation");
        assert_eq!(opts, Opts::new("pwsh"));
        assert!(opts.cols.is_none());
        assert!(opts.rows.is_none());
        assert!(opts.cwd.is_none());
        assert!(opts.env.is_none());
    }

    #[test]
    fn des_dimensions_absentes_vautent_pas_zero() {
        // Piege `?` contre `??` : une taille absente demande la taille par
        // defaut du pseudo terminal, alors que 0 demande une taille nulle.
        let opts: Opts = serde_json::from_str(r#"{"name":"bash"}"#).expect("deserialisation");
        assert_ne!(opts.cols, Some(0), "zero n'est pas la valeur par defaut");
        assert_ne!(opts.rows, Some(0), "zero n'est pas la valeur par defaut");
    }

    #[test]
    fn un_environnement_vide_et_un_environnement_absent_sont_deux_choses_differentes() {
        // Un objet vide demande un processus sans aucune variable, une cle
        // absente demande l heritage de l'environnement du parent.
        let vide: Opts = serde_json::from_str(r#"{"name":"bash","env":{}}"#).expect("deserialisation");
        let absent: Opts = serde_json::from_str(r#"{"name":"bash"}"#).expect("deserialisation");
        assert_eq!(vide.env, Some(BTreeMap::new()));
        assert_eq!(absent.env, None);
        assert_ne!(vide.env, absent.env);
    }

    #[test]
    fn les_cles_de_l_environnement_sont_triees_et_non_poses_dans_l_ordre() {
        let json = r#"{"name":"bash","env":{"ZOO":"3","ALPHA":"1","MILE":"2"}}"#;
        let opts: Opts = serde_json::from_str(json).expect("deserialisation");
        let attendu: BTreeMap<String, String> = BTreeMap::from([
            ("ALPHA".to_string(), "1".to_string()),
            ("MILE".to_string(), "2".to_string()),
            ("ZOO".to_string(), "3".to_string()),
        ]);
        assert_eq!(opts.env, Some(attendu));
        // La sortie est donc reecrite dans l'ordre trie, pas dans celui du JSON.
        let reecrit = serde_json::to_string(&opts).expect("serialisation");
        assert!(reecrit.contains(r#""env":{"ALPHA":"1","MILE":"2","ZOO":"3"}"#), "{}", reecrit);
    }

    #[test]
    fn les_noms_de_champs_des_options_sont_bien_ceux_de_la_source() {
        let opts = Opts {
            name: "bash".to_string(),
            cols: Some(100),
            rows: Some(40),
            cwd: Some("/tmp".to_string()),
            env: Some(BTreeMap::from([("TERM".to_string(), "xterm".to_string())])),
        };
        let json = serde_json::to_string(&opts).expect("serialisation");
        assert_eq!(
            json,
            r#"{"name":"bash","cols":100,"rows":40,"cwd":"/tmp","env":{"TERM":"xterm"}}"#
        );
        let relu: Opts = serde_json::from_str(&json).expect("deserialisation");
        assert_eq!(relu, opts);
    }

    // -- Exit ---------------------------------------------------------------

    #[test]
    fn une_sortie_sans_signal_ne_donne_que_le_code_de_sortie() {
        let sortie = Exit {
            exit_code: 137,
            signal: None,
        };
        let json = serde_json::to_string(&sortie).expect("serialisation");
        assert_eq!(json, r#"{"exitCode":137}"#);
    }

    #[test]
    fn le_code_de_sortie_sort_sous_le_nom_exit_code_avec_sa_majuscule() {
        // Piege des noms de champs : `exitCode` et non `exit_code`.
        let sortie = Exit {
            exit_code: 0,
            signal: None,
        };
        let json = serde_json::to_string(&sortie).expect("serialisation");
        assert!(json.contains("\"exitCode\""), "{}", json);
        assert!(!json.contains("exit_code"), "{}", json);
        let relu: Exit = serde_json::from_str(r#"{"exitCode":0}"#).expect("deserialisation");
        assert_eq!(relu.exit_code, 0);
        assert_eq!(relu.signal, None);
    }

    #[test]
    fn un_signal_du_json_peut_etre_un_nombre_ou_un_nom() {
        let numerique: Exit =
            serde_json::from_str(r#"{"exitCode":137,"signal":9}"#).expect("deserialisation");
        assert_eq!(numerique.signal, Some(Signal::Nombre(9)));
        let textuel: Exit =
            serde_json::from_str(r#"{"exitCode":137,"signal":"SIGKILL"}"#).expect("deserialisation");
        assert_eq!(textuel.signal, Some(Signal::Nom("SIGKILL".to_string())));
    }

    #[test]
    fn un_signal_a_zero_est_present_et_un_signal_absent_reste_absent() {
        // Piege `?` contre `??` : 0 est une valeur presente, il ne doit pas
        // etre confondu avec une cle absente.
        let avec_zero: Exit =
            serde_json::from_str(r#"{"exitCode":0,"signal":0}"#).expect("deserialisation");
        assert_eq!(avec_zero.signal, Some(Signal::Nombre(0)));
        let sans: Exit = serde_json::from_str(r#"{"exitCode":0}"#).expect("deserialisation");
        assert_eq!(sans.signal, None);
    }

    #[test]
    fn un_signal_textuel_vide_survaut_il_est_different_d_un_signal_absent() {
        let vide: Exit =
            serde_json::from_str(r#"{"exitCode":1,"signal":""}"#).expect("deserialisation");
        assert_eq!(vide.signal, Some(Signal::Nom(String::new())));
        let absent: Exit = serde_json::from_str(r#"{"exitCode":1}"#).expect("deserialisation");
        assert_eq!(absent.signal, None);
    }

    // -- Proc ---------------------------------------------------------------

    #[test]
    fn un_pseudo_terminal_enregistre_ecriture_redimensionnement_et_arret() {
        let proc = ProcFaux::nouveau();
        let journal = Arc::clone(&proc.journal);
        let proc: Box<dyn Proc> = Box::new(proc);
        proc.write("ls");
        proc.resize(100, 40);
        assert_eq!(
            *verrouiller(&journal),
            vec![
                "write(ls)".to_string(),
                "resize(100,40)".to_string()
            ]
        );
    }

    #[test]
    fn un_arret_sans_signal_n_est_pas_un_arret_avec_un_nom_vide() {
        // Piege `?` contre `??` : `kill()` et `kill("")` sont deux appels
        // differents en TypeScript et le portage doit les distinguer.
        let proc = ProcFaux::nouveau();
        let journal = Arc::clone(&proc.journal);
        let proc: Box<dyn Proc> = Box::new(proc);
        proc.kill(None);
        proc.kill(Some(""));
        proc.kill(Some("SIGKILL"));
        assert_eq!(
            *verrouiller(&journal),
            vec![
                "kill(aucun)".to_string(),
                "kill()".to_string(),
                "kill(SIGKILL)".to_string()
            ]
        );
    }

    #[test]
    fn un_ecouteur_de_donnees_recoit_les_morceaux_puis_se_desabonne() {
        let proc = ProcFaux::nouveau();
        let abonne = Arc::clone(&proc.abonne);
        let recus = Arc::new(Mutex::new(Vec::new()));
        let recus_forts = Arc::clone(&recus);
        let proc: Box<dyn Proc> = Box::new(proc);
        assert!(abonne.load(Ordering::SeqCst), "l'abonnement est actif au depart");
        let poignet = proc.on_data(Box::new(move |morceau: &str| {
            verrouiller(&recus_forts).push(morceau.to_string());
        }));
        assert_eq!(
            *verrouiller(&recus),
            vec!["premier morceau".to_string(), "second morceau".to_string()]
        );
        poignet.dispose();
        assert!(
            !abonne.load(Ordering::SeqCst),
            "l'abonnement doit etre retire apres dispose"
        );
    }

    #[test]
    fn un_ecouteur_de_sortie_recoit_le_code_et_le_signal() {
        let proc = ProcFaux::nouveau();
        let sorties = Arc::new(Mutex::new(Vec::new()));
        let sorties_fortes = Arc::clone(&sorties);
        let proc: Box<dyn Proc> = Box::new(proc);
        let poignet = proc.on_exit(Box::new(move |evenement: Exit| {
            verrouiller(&sorties_fortes).push(evenement);
        }));
        let attendu = Exit {
            exit_code: 137,
            signal: Some(Signal::Nombre(9)),
        };
        assert_eq!(*verrouiller(&sorties), vec![attendu]);
        poignet.dispose();
    }

    #[test]
    fn le_pid_du_pseudo_terminal_est_lisible_tant_qu_il_est_ouvert() {
        let proc = ProcFaux::nouveau();
        let proc: Box<dyn Proc> = Box::new(proc);
        assert_eq!(proc.pid(), 4242);
    }
}
