//! Portage de `packages/core/src/pty/pty.node.ts`.
//!
//! # Ce que contient la source
//!
//! Vingt-neuf lignes, dont six portent du comportement :
//!
//! ```ts
//! import * as pty from "@lydell/node-pty"
//! import type { Opts, Proc } from "./pty"
//!
//! export type { Disp, Exit, Opts, Proc } from "./pty"
//!
//! export function spawn(file: string, args: string[], opts: Opts): Proc {
//!   const proc = pty.spawn(file, args, {
//!     ...opts,
//!     ...(process.platform === "win32" ? { useConptyDll: true } : {}),
//!   })
//!   return { pid: proc.pid, onData(l) { return proc.onData(l) }, /* ... */ }
//! }
//! ```
//!
//! Trois choses y sont observables, et seulement trois :
//!
//! 1. le **melange d options** : les cinq options du appelant, puis, sur
//!    Windows seulement, `useConptyDll: true` ;
//! 2. l appel `pty.spawn(file, args, options)`, ou un processus est reellement
//!    cree ;
//! 3. l objet rendu, qui **recopie** `pid` et **delegue** les cinq autres
//!    operations sans les modifier.
//!
//! # Une implementation ALTERNATIVE, pas un deuxieme contrat
//!
//! Le point le plus important de ce fichier : il ne declare **aucun** type.
//! `Opts`, `Proc`, `Exit` et `Disp` sont deja portes par `pty_pty.rs`, qui les
//! importe de `packages/core/src/pty/pty.ts`, et ils sont ici **reimportes** :
//!
//! ```ignore
//! pub use crate::swarm::pty_pty::{Disp, Exit, Opts, Proc};
//! ```
//!
//! C'est la ligne `export type { Disp, Exit, Opts, Proc } from "./pty"` de la
//! source, qui n est qu un reexport. La recopier en Rust produirait deux
//! definitions divergentes du meme contrat, et **Rust ne le signale pas** : deux
//! `struct Opts` dans deux modules compilent ensemble sans le moindre
//! avertissement, alors qu elles decrivent deux choses differentes. Deux
//! fonctions `spawn` identiques mais acceptant chacune leur propre `Opts` ne
//! peuvent meme plus etre interchangeables. La relecture doit donc verifier que
//! ce fichier ne contient ni `struct` ni `enum` portant l un de ces quatre noms.
//!
//! Ce fichier est le pendant de `pty.node.ts` : l adaptateur **Node** du
//! contrat, a cote du contrat lui-meme (`pty_pty.rs`) et du transport
//! (`pty_protocol.rs`).
//!
//! # Pourquoi `pty.spawn` n est pas porte
//!
//! Un pseudo terminal reel se fabrique par un **appel systeme Windows** :
//! `CreatePseudoConsole` (ConPTY) exporte par `kernel32.dll`, son
//! `ResizePseudoConsole`, son `ClosePseudoConsole`, et la paire de tubes
//! associee qui fait la circulation dans les deux sens. Aucun de ces symboles
//! n existe dans la bibliotheque standard de Rust, et la regle du portage
//! interdit de modifier `Cargo.toml`, donc d ajouter la dependance qui les
//! exposerait. Concretement, il manque aussi tout ce qui va avec : une tache de
//! lecture bloquante, une temporisation d entrees-sorties, et la conversion
//! des octets en texte.
//!
//! Une approximation par `std::process::Command` a ete **refusee** et c est
//! delibere. `Command` ouvre des **tubes**, pas une console : le fils n aurait
//! ni entree standard interactive, ni `isatty` vrai, donc `resize` n aurait
//! **aucun effet** et `write` n ateindrait jamais un terminal. Une API qui
//! promet de redimensionner sans redimensionner est plus dangereuse qu une API
//! absente, parce qu elle laisse croire a un terminal qui n existe pas.
//!
//! Ce qui est donc porte : le **calcul** des options, c est a dire le melange,
//! seule expression non triviale de la source, et la **delegation**, c est a
//! dire les six membres de l objet rendu. La creation est **injectee** par
//! l appelant : [`spawn`] recoit l implementation en dernier parametre. Ce n est
//! pas un allongement gratuit, c est ce qui rend la delegation testable sans
//! processus, sans tache de fond et sans terminal.
//!
//! # Pourquoi il n y a aucune branche POSIX dans ce fichier
//!
//! La source ecrit `process.platform === "win32" ? { useConptyDll: true } : {}`.
//! Sur la seule cible du projet, la condition est **toujours vraie** : le
//! ternaire n a donc pas de branche `else` a porter, et il n y a pas un seul
//! `#[cfg(unix)]` dans ce fichier. Les formes de sortie de type Unix
//! (`forkpty`, `posix_openpt`, `ptsname`) ne sont donc **pas** mentionnees : les
//! porter serait du code mort qui ne compiles jamais. C est aussi pour cela
//! qu aucun separateur `/` n est suppose : la seule cible etant Windows, un
//! chemin s ecrit avec des antislashs, et ce fichier ne normalise rien.
//!
//! # Le piege du melange : la cle imposee passe en DERNIER
//!
//! `{ ...opts, ...(win32 ? { useConptyDll: true } : {}) }` se lit dans l ordre :
//! les options du appelant d abord, puis une cle imposee. En JavaScript, une
//! diffusion ecrase tout ce qui porte le meme nom, donc `useConptyDll: true`
//! **gagne** meme si l objet du appelant portait deja cette cle. Le portage
//! conserve cet ordre : [`DemandeNodePty::resolve`] lit la valeur entrante puis
//! l ecrase, et le test `une_cle_imposee_ecrase_la_valeur_du_callerant` le
//! verifie.
//!
//! Deux consequences moins evidentes :
//!
//! - La branche `else` diffuse `{}`, c est a dire **rien**. Le portage n a donc
//!   aucun cas "drapeaux absents" a representer : sur cette cible, le drapeau
//!   est toujours pose.
//! - `useConptyDll` est la **seule** cle de la famille en camelCase. Les cinq
//!   autres (`name`, `cols`, `rows`, `cwd`, `env`) sont des mots uniques en
//!   minuscules et passent inapercus ; celle-ci ne peut pas etre orthographiee
//!   `use_conpty_dll` du cote du nom echange, faute de voisin qui la consomme.
//!   Elle est donc portee sous le nom exact de node-pty, en constante
//!   [`CLE_USE_CONPTY_DLL`], et le champ Rust qui la porte est
//!   `use_conpty_dll`.
//!
//! # Le trou du typage structurel, seul endroit ou le supplement sert
//!
//! `Opts` ne declare **pas** `useConptyDll`, et pourtant la source force cette
//! cle. Ce n est possible que parce qu une diffusion `{ ...opts }` copie les
//! proprietes **presentes a l execution**, declarees ou non : le typage
//! structurel de TypeScript laisse passer les proprietes en trop. En Rust,
//! `Opts` est une `struct` fermee, donc il n y a nulle part ou faire ecraser.
//! Le portage rend ce trou visible plutot que de l effacer : [`Supplement`] est
//! la projection des proprietes que le type declare ne voit pas, et elle n est
//! lue que pour `useConptyDll`.
//!
//! Pour les cinq cles declarees, c est le champ type de `Opts` qui fait foi :
//! une entree du supplement portant `cols` ou `cwd` est ignoree, car c est la
//! seule facon de garantir que le supplement ne devient pas une deuxieme source
//! de verite. Toute autre cle du supplement est ignoree elle aussi : la source
//! la transmet a node-pty, mais le code du depot n en pose aucune (`encoding`
//! reste a son defaut), donc il n y a rien a porter.
//!
//! # `file ? file : name` : un test de veracite, pas de nullite
//!
//! La source passe le programme **deux fois** : par le parametre `file` et par
//! `opts.name`. node-pty en garde un seul et l autre sert de repli. Ce repli est
//! un test de **veracite** (`file` non vide), pas un test de nullite : en
//! JavaScript, une chaine vide est fausse, donc `file = ""` retombe sur
//! `opts.name`, alors que `" "` ou `"0"` sont des noms valides et ne retombent
//! pas. [`fichier_de_lancement`] fait exactement ce test la.
//!
//! # `pid` est une photographie, pas un acesseur
//!
//! `return { pid: proc.pid, ... }` **recopie** la valeur dans un objet
//! litteral. Ce n est pas une delegation : si la valeur changeait un jour, l
//! objet rendu garderait l ancienne. Le portage garde cette copie, capturee
//! dans [`Adaptateur::nouveau`], et le test
//! `le_pid_est_une_photographie_prise_au_demarrage` le verifie en changeant la
//! valeur de l implementation **apres** la construction. C est le seul ecart
//! entre la delegation de la source et le trait [`Proc`], qui expose `pid()`
//! comme une methode.
//!
//! # Le piege `?` contre `??` est dans `kill`
//!
//! La source ecrit `kill(signal) { proc.kill(signal) }`, ou `signal` peut
//! valoir `undefined`. Transmettre `undefined` et transmettre `""` sont deux
//! appels differents : `""` est un nom de signal invalide, alors que l absence
//! demande le signal par defaut de l implementation. Le portage forwarde
//! l [`Option`] tel quel, sans jamais ecrire `unwrap_or_default`, et le test
//! `un_signal_absent_arrive_absent_et_un_nom_vide_arrive_vide` verrouille les
//! deux cas sur l implementation de dessous.
//!
//! Rappel du piege voisin : `cols` et `rows` absents dans `Opts` restent
//! absents dans [`DemandeNodePty`]. Ils ne sont jamais remplaces par `0`, qui
//! demanderait une taille nulle ; un `0` est une **valeur presente**, il ne
//! doit pas disparaitre non plus. `pty_pty.rs` le verifie deja au niveau du
//! contrat, ici c est verifie au niveau de la demande.

use std::collections::BTreeMap;
use std::fmt;

// Reexport de la source : `export type { Disp, Exit, Opts, Proc } from "./pty"`.
// Ces quatre noms appartiennent a `pty_pty.rs`, ils ne sont pas redefinis ici.
pub use crate::swarm::pty_pty::{Disp, Exit, Opts, Proc};
use crate::swarm::pty_pty::{DataListener, ExitListener};

/// Nom exact, en camelCase, de l option node-pty posee par la source.
///
/// C'est la seule cle camelCase de la famille : `name`, `cols`, `rows`, `cwd` et
/// `env` sont des mots uniques en minuscules, donc invisibles a la relecture.
/// Celle-ci ne l'est pas, et une faute d'orthographe passerait la compilation,
/// puisque la valeur ne transite par aucune structure serialisee.
pub const CLE_USE_CONPTY_DLL: &str = "useConptyDll";

/// Proprietes que le type `Opts` ne declare pas, mais que la diffusion
/// `{ ...opts }` de la source copie quand meme.
///
/// C'est la projection du trou du typage structurel de TypeScript, decrit dans
/// la note du module. Elle n'est lue que pour [`CLE_USE_CONPTY_DLL`] : les cinq
/// cles declarees par [`Opts`] font foi, et toute autre cle est ignoree, la
/// source n'en posant aucune.
pub type Supplement = BTreeMap<String, OptionBrute>;

/// Valeur brute d une propriete d option, tous types confondus.
///
/// `node-pty` n'a pas de type unique pour ses options : `cols` est un nombre,
/// `cwd` une chaine, `useConptyDll` un booleen. Un `enum` tient les trois sans
/// pretendre qu'elles sont interchangeables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionBrute {
    /// Valeur textuelle, comme `cwd` ou un nom d executable.
    Texte(String),
    /// Valeur numerique entiere, comme `cols` ou `rows`.
    Nombre(u32),
    /// Valeur booleenne, comme [`CLE_USE_CONPTY_DLL`].
    Booleen(bool),
}

/// La demande de demarrage remise a node-pty, une fois le melange de la source
/// applique.
///
/// C'est le resultat de `{ ...opts, ...(win32 ? { useConptyDll: true } : {}) }`,
/// complete par le couple `(file, args)` que la source passe separement. Elle
/// regroupe ce que `pty.spawn` recoit en trois parametres, sans en changer
/// l'ordre de lecture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandeNodePty {
    /// Programme a lancer, **apres** application du repli sur `opts.name` par
    /// [`fichier_de_lancement`].
    pub fichier: String,
    /// Arguments de la ligne de commande, recopies tels quels, sans citation
    /// ni normalisation. La source ne cite rien non plus.
    pub args: Vec<String>,
    /// Colonnes, absentes pour la taille par defaut, jamais remplacees par `0`.
    pub colonnes: Option<u32>,
    /// Lignes, absentes pour la taille par defaut, jamais remplacees par `0`.
    pub lignes: Option<u32>,
    /// Repertoire de travail, absent pour heriter de celui du processus
    /// courant. Recopie sans normalisation : sur cette cible, un chemin
    /// s'ecrit avec des antislashs et le portage n'en introduit aucun.
    pub repertoire: Option<String>,
    /// Environnement du fils, absent pour heriter de celui du processus
    /// courant. Une table vide reste une table vide : l absence et la table
    /// vide sont deux demandes differentes, comme dans `pty_pty.rs`.
    pub environnement: Option<BTreeMap<String, String>>,
    /// Drapeau ConPTY, toujours `true` sur cette cible. La source le pose en
    /// dernier, donc il gagne meme si le [`Supplement`] en portait un autre.
    pub use_conpty_dll: bool,
}

impl DemandeNodePty {
    /// Applique le melange de la source, comme le fait l'objet d'options passe
    /// a `pty.spawn`.
    ///
    /// L'ordre est celui de la diffusion et il est significatif : les cinq
    /// options declarees viennent de [`Opts`], puis la cle imposee est posee
    /// par dessus, ecrasant tout ce qui porterait le meme nom.
    pub fn resolve(fichier: &str, args: &[String], opts: &Opts, supplement: &Supplement) -> Self {
        Self {
            fichier: fichier_de_lancement(fichier, &opts.name),
            args: args.to_vec(),
            colonnes: opts.cols,
            lignes: opts.rows,
            repertoire: opts.cwd.clone(),
            environnement: opts.env.clone(),
            use_conpty_dll: impose_use_conpty_dll(supplement),
        }
    }

    /// Le drapeau ConPTY, sous le nom exact qu'attend node-pty.
    ///
    /// Le nom de la source est porte par [`CLE_USE_CONPTY_DLL`], celui du champ
    /// par `use_conpty_dll` ; la conversion entre les deux est faite ici, une
    /// fois, plutot que laissee a l appelant.
    pub fn drapeaux_pour_node_pty(&self) -> Vec<(&'static str, OptionBrute)> {
        vec![(CLE_USE_CONPTY_DLL, OptionBrute::Booleen(self.use_conpty_dll))]
    }
}

/// Pose la cle imposee par la source, qui passe apres la diffusion des options
/// du appelant et gagne donc toujours.
///
/// La variante du [`Supplement`] est relue puis ecrasee : c'est exactement ce
/// que fait `{ ...opts, ...(win32 ? { useConptyDll: true } : {}) }`. Le `match`
/// est volontairement exhaustif, pour que l'ajout d'une variante a
/// [`OptionBrute`] force a reflechir a cette priorite.
fn impose_use_conpty_dll(supplement: &Supplement) -> bool {
    match supplement.get(CLE_USE_CONPTY_DLL) {
        // La cle etait deja la, dans n'importe quel type : elle est ecrasee
        // quand meme, la diffusion imposee venant apres celle de l'objet.
        Some(OptionBrute::Booleen(_))
        | Some(OptionBrute::Nombre(_))
        | Some(OptionBrute::Texte(_)) => true,
        // Aucune cle de ce nom : la source en pose une, et c'est `true`.
        None => true,
    }
}

/// Programme reellement lance, comme le repli de node-pty sur `opts.name`.
///
/// La source passe le programme deux fois : par `file` et par `opts.name`.
/// node-pty en retient un et l'autre sert de repli.
///
/// # Portee de cette affirmation
///
/// Ce repli est **deduit** de l'usage de la bibliotheque, pas verifie ici :
/// `@lydell/node-pty` (version `1.2.0-beta.12` d'apres le catalogue de
/// dependances) n'est pas installee sur cette machine, donc son code n'est pas
/// consultable. Ce qui est verifie, en revanche, c'est la forme de l'appel : la
/// source passe bien les deux. Si node-pty resolvait autrement, seule cette
/// fonction serait a revoir, le reste du fichier n'en depend pas.
///
/// Le test est un test de **veracite**, comme `file ? file : optfile` : une
/// chaine vide est fausse et fait tomber sur le repli, une chaine faite
/// d'espaces est vraie et ne retombe pas.
pub fn fichier_de_lancement(fichier: &str, nom_des_options: &str) -> String {
    if fichier.is_empty() {
        nom_des_options.to_string()
    } else {
        fichier.to_string()
    }
}

/// L'objet rendu par [`spawn`] : le `Proc` de la source.
///
/// Il ne fait que deux choses : **recopier** le `pid` au moment de la
/// construction, puis **deleguer** les cinq autres operations a
/// l'implementation injectee. Aucun etat n'est ajoute, aucune operation n'est
/// interpretee.
///
/// Le type est generique sur l'implementation, ce qui n'a pas d'equivalent
/// TypeScript : en JavaScript, `pty.spawn` rend un objet d'une classe interne
/// et la fonction n'a pas a le nommer. Ici, nommer la classe permet de la
/// substituer dans les tests, ce qui est la seule facon de verifier la
/// delegation sans ConPTY.
pub struct Adaptateur<P: Proc> {
    /// Valeur de `pid` au moment de la construction, et non au moment de
    /// l'appel : la source la recopie dans un objet litteral.
    pid: u32,
    /// Ce que la source a passe a `pty.spawn`, une fois melange.
    demande: DemandeNodePty,
    /// L'objet `Proc` sous-jacent, jamais enveloppe ni interprete.
    interne: P,
}

impl<P: Proc> Adaptateur<P> {
    /// Enveloppe une implementation, en photographiant son `pid`.
    ///
    /// L'ordre compte : le `pid` est lu **avant** que l'implementation ne soit
    /// deplacee dans la structure, et l'objet litteral de la source fait de
    /// meme.
    pub fn nouveau(interne: P, demande: DemandeNodePty) -> Self {
        let pid = interne.pid();
        Self {
            pid,
            demande,
            interne,
        }
    }

    /// La demande de demarrage, telle que la source l'a remise a node-pty.
    ///
    /// Cette inspection n'existe pas en amont : la source jette l'objet
    /// d'options apres l'avoir passe a `pty.spawn` et ne rend que le `Proc`.
    /// Elle est portee parce que c'est elle, et non la delegation, qui porte le
    /// comportement du fichier.
    pub fn demande(&self) -> &DemandeNodePty {
        &self.demande
    }

    /// L'implementation sous-jacente, pour un appelant qui aurait besoin de
    /// parler ConPTY directement.
    pub fn implementation(&self) -> &P {
        &self.interne
    }
}

impl<P: Proc> fmt::Debug for Adaptateur<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Adaptateur")
            .field("pid", &self.pid)
            .field("demande", &self.demande)
            .field("interne", &"<implementation>")
            .finish()
    }
}

/// Les six operations de la source, deleguees une a une.
///
/// Aucune n est reecrite : chacune transmet ses arguments et rend ce que
/// l'implementation rend, y compris `pid`, qui ne vient **pas** de la
/// delegation mais de la copie faite a la construction.
impl<P: Proc> Proc for Adaptateur<P> {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn on_data(&self, listener: DataListener) -> Box<dyn Disp> {
        self.interne.on_data(listener)
    }

    fn on_exit(&self, listener: ExitListener) -> Box<dyn Disp> {
        self.interne.on_exit(listener)
    }

    fn write(&self, data: &str) {
        self.interne.write(data);
    }

    fn resize(&self, cols: u32, rows: u32) {
        self.interne.resize(cols, rows);
    }

    /// L'absence est transmise **comme absence**.
    ///
    /// `signal` ne traverse aucun `unwrap_or_default` : `None` reste `None` et
    /// `Some("")` reste une chaine vide. Ce sont deux appels differents en
    /// JavaScript, ou le second envoie `""` et le premier envoie `undefined`.
    fn kill(&self, signal: Option<&str>) {
        self.interne.kill(signal);
    }
}

/// Demarre un pseudo terminal Node, comme `spawn(file, args, opts)`.
///
/// # Ce qui change par rapport a la source
///
/// La source cree le pseudo terminal, puis l'enveloppe. Ici la creation est
/// **injectee** : l'implementation ConPTY est le dernier parametre. C'est la
/// seule adaptation de la signature, et elle est inevitable :
/// `CreatePseudoConsole` n'est pas dans la bibliotheque standard et
/// `Cargo.toml` n'est pas modifiable. Voir la note du module pour le refus
/// explicite d'une approximation par `std::process::Command`.
///
/// L'enveloppe, elle, est portee telle quelle : meme `pid` photographie, memes
/// cinq delegations.
///
/// # Exemple
///
/// ```
/// use ycode::swarm::pty_node::DemandeNodePty;
/// use ycode::swarm::pty_pty::Opts;
///
/// let demande = DemandeNodePty::resolve(
///     "C:\\projet\\pwsh.exe",
///     &["-NoLogo".to_string()],
///     &Opts::new("pwsh"),
///     &Default::default(),
/// );
/// assert!(demande.use_conpty_dll);
/// assert_eq!(demande.args, vec!["-NoLogo".to_string()]);
/// assert_eq!(demande.colonnes, None, "absent ne veut pas dire zero");
/// ```
pub fn spawn<P: Proc>(
    fichier: &str,
    args: &[String],
    opts: &Opts,
    implementation: P,
) -> Adaptateur<P> {
    let demande = DemandeNodePty::resolve(fichier, args, opts, &Supplement::new());
    Adaptateur::nouveau(implementation, demande)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::{Cell, RefCell};
    use std::sync::OnceLock;

    // -- doubles d'essai ----------------------------------------------------
    //
    // Aucune concession a la concurrence n'est faite ici : ni tache de fond,
    // ni `Mutex`, ni `Condvar`, ni temporisation, et surtout aucun processus
    // n'est cree. Le journal du faux terminal est un `RefCell`, qui ne peut pas
    // bloquer et ne bloque jamais puisque chaque test est mono tache et
    // qu'aucun appel d'ecouteur n'imbrique un emprunt.
    //
    // Les ecouteurs, eux, sont contraints par le contrat : `DataListener` est
    // un `Box<dyn FnMut(&str) + Send + 'static>`. Un `RefCell` local ne peut
    // donc pas etre capture, il ne vit pas assez longtemps et n'est pas
    // partageable. D'ou les `OnceLock` installes sur une adresse
    // `'static` par `Box::leak` : une ecriture concurrente y serait traitee
    // par la bibliotheque, et le test n'en demande aucune.

    /// Abonnement jetable, sans etat : la source renvoie l'objet de
    /// node-pty, dont le retrait n'est pas observe par ce fichier.
    struct AbonnementFaux;

    impl Disp for AbonnementFaux {
        fn dispose(&self) {}
    }

    /// Etat note par le faux terminal, que le test relit apres l'avoir passe
    /// a [`spawn`].
    struct EtatFaux {
        journal: RefCell<Vec<String>>,
        pid: Cell<u32>,
    }

    /// Implementation d'essai du trait `Proc`, qui ne simule ni terminal ni
    /// processus : elle note les appels et distribue immediatement les
    /// ecouteurs, parce qu'il n'y a rien derriere.
    ///
    /// C'est une **poignee** vers un [`EtatFaux`] partage, et non l'etat
    /// lui-meme. [`spawn`] consomme l'implementation qu'on lui remet, et le
    /// test doit malgre tout relire le journal et le pid apres la
    /// construction, pour verifier que la delegation agit et que le `pid`, lui,
    /// est une photographie. Une poignee `Copy` rend l'etat observable par les
    /// deux cotes, sans que le test ait a cloner quoi que ce soit.
    #[derive(Clone, Copy)]
    struct FauxPty(&'static EtatFaux);

    impl FauxPty {
        fn nouveau() -> Self {
            // L'etat est vole dans une adresse statique : il vit jusqu'a la fin
            // du programme de test, comme les `OnceLock`.installes plus bas par
            // `Box::leak`. Aucun partage entre taches n'est demande, donc
            // `Rc` n'aurait rien a securiser.
            Self(Box::leak(Box::new(EtatFaux {
                journal: RefCell::new(Vec::new()),
                pid: Cell::new(4242),
            })))
        }

        fn journal(&self) -> Vec<String> {
            self.0.journal.borrow().clone()
        }
    }

    impl Proc for FauxPty {
        fn pid(&self) -> u32 {
            self.0.pid.get()
        }

        fn on_data(&self, mut listener: DataListener) -> Box<dyn Disp> {
            listener("premier morceau");
            listener("second morceau");
            Box::new(AbonnementFaux)
        }

        fn on_exit(&self, mut listener: ExitListener) -> Box<dyn Disp> {
            listener(Exit {
                exit_code: 137,
                signal: None,
            });
            Box::new(AbonnementFaux)
        }

        fn write(&self, data: &str) {
            self.0.journal.borrow_mut().push(format!("write({})", data));
        }

        fn resize(&self, cols: u32, rows: u32) {
            self.0
                .journal
                .borrow_mut()
                .push(format!("resize({},{})", cols, rows));
        }

        fn kill(&self, signal: Option<&str>) {
            // L'absence et la chaine vide doivent rester deux appels
            // distinguables dans le journal, donc le marqueur est explicite.
            let entree = match signal {
                None => "kill(aucun)".to_string(),
                Some(nom) => format!("kill({})", nom),
            };
            self.0.journal.borrow_mut().push(entree);
        }
    }

    /// Une demande minimale, avec un chemin Windows ecrit a l'ancienne.
    fn demande_de_test() -> DemandeNodePty {
        DemandeNodePty::resolve(
            "C:\\projet\\pwsh.exe",
            &[],
            &Opts::new("pwsh"),
            &Supplement::new(),
        )
    }

    // -- le reexport --------------------------------------------------------

    #[test]
    fn les_quatre_types_reexportes_sont_ceux_du_contrat_et_non_des_copies() {
        // La preuve est statique : si ce fichier redefinissait `Opts` ou `Proc`,
        // la fonction ci-dessous refuserait de compiler. C'est tout l'interet du
        // reexport, et c'est invisible a la relecture d'un simple diff.
        fn preuve(_abonnement: &dyn Disp, _sortie: Exit, _opts: Opts, _proc: &dyn Proc) {}

        let faux = FauxPty::nouveau();
        preuve(
            &AbonnementFaux,
            Exit {
                exit_code: 0,
                signal: None,
            },
            Opts::new("pwsh"),
            &faux,
        );
    }

    // -- le melange d'options -----------------------------------------------

    #[test]
    fn le_nom_de_la_cle_conpty_est_en_camelcase() {
        // Piege des noms : la source ecrit `useConptyDll`. Un nom en minuscules
        // passerait la compilation, car la valeur ne transite par aucun JSON.
        assert_eq!(CLE_USE_CONPTY_DLL, "useConptyDll");
    }

    #[test]
    fn la_demande_pose_toujours_le_drapeau_conpty() {
        // La branche `else` de la source diffuse `{}`, donc elle ne pose rien.
        // Sur cette cible, il n'y a donc aucun cas "drapeau absent" a
        // representer, et rien ne doit permettre de le retrouver.
        assert!(demande_de_test().use_conpty_dll);
    }

    #[test]
    fn une_cle_imposee_ecrase_la_valeur_du_callerant() {
        // L'ordre de la diffusion est le comportement du fichier : la cle posee
        // en dernier gagne. Sans cela, le drapeau pourrait heriter de la
        // valeur du supplement.
        let faux_faux = Supplement::from([(
            CLE_USE_CONPTY_DLL.to_string(),
            OptionBrute::Booleen(false),
        )]);
        let demande = DemandeNodePty::resolve("pwsh.exe", &[], &Opts::new("pwsh"), &faux_faux);
        assert!(demande.use_conpty_dll, "la cle imposee passe en dernier");

        // Et le drapeau est bien transmis a node-pty sous le nom de la source.
        assert_eq!(
            demande.drapeaux_pour_node_pty(),
            vec![(CLE_USE_CONPTY_DLL, OptionBrute::Booleen(true))]
        );
    }

    #[test]
    fn les_cinq_options_declarees_sont_recopiees_telles_elles() {
        let env = BTreeMap::from([("TERM".to_string(), "xterm-256color".to_string())]);
        let opts = Opts {
            name: "pwsh".to_string(),
            cols: Some(120),
            rows: Some(40),
            cwd: Some("C:\\projet\\demo".to_string()),
            env: Some(env.clone()),
        };
        let demande = DemandeNodePty::resolve("pwsh.exe", &[], &opts, &Supplement::new());
        assert_eq!(demande.colonnes, Some(120));
        assert_eq!(demande.lignes, Some(40));
        assert_eq!(demande.repertoire.as_deref(), Some("C:\\projet\\demo"));
        assert_eq!(demande.environnement, Some(env));
    }

    #[test]
    fn une_option_non_declaree_ne_deplace_pas_un_champ_declare() {
        // Le supplement ne sert qu'a `useConptyDll`. Si une entree portait le
        // nom d'une cle declaree, ce serait une seconde source de verite, donc
        // elle est ignoree et le champ type fait foi.
        let supplement = Supplement::from([
            ("cols".to_string(), OptionBrute::Nombre(999)),
            ("cwd".to_string(), OptionBrute::Texte("C:\\ailleurs".to_string())),
        ]);
        let opts = Opts {
            name: "pwsh".to_string(),
            cols: Some(80),
            rows: Some(24),
            cwd: Some("C:\\ici".to_string()),
            env: None,
        };
        let demande = DemandeNodePty::resolve("pwsh.exe", &[], &opts, &supplement);
        assert_eq!(demande.colonnes, Some(80), "le champ declare fait foi");
        assert_eq!(demande.repertoire.as_deref(), Some("C:\\ici"));
    }

    #[test]
    fn une_option_brute_de_mauvais_type_ne_remplace_pas_une_option_valide() {
        // `cols` est un nombre dans `Opts` : un supplement qui le porterait en
        // tant que chaine est ignore, il ne fabrique pas de conversion.
        let supplement =
            Supplement::from([("cols".to_string(), OptionBrute::Texte("80".to_string()))]);
        let demande = DemandeNodePty::resolve("pwsh.exe", &[], &Opts::new("pwsh"), &supplement);
        assert_eq!(demande.colonnes, None, "le supplement reste ignore");
    }

    #[test]
    fn des_dimensions_absentes_restent_absentes_et_un_zero_reste_un_zero() {
        // Piege `?` contre `??` : absent ne veut pas dire zero, et zero est une
        // valeur presente qui ne doit pas disparaitre.
        let sans = demande_de_test();
        assert_eq!(sans.colonnes, None);
        assert_eq!(sans.lignes, None);
        assert_ne!(sans.colonnes, Some(0));

        let avec_zero = Opts {
            name: "pwsh".to_string(),
            cols: Some(0),
            rows: Some(0),
            cwd: None,
            env: None,
        };
        let demande = DemandeNodePty::resolve("pwsh.exe", &[], &avec_zero, &Supplement::new());
        assert_eq!(demande.colonnes, Some(0), "zero demande une taille nulle");
        assert_eq!(demande.lignes, Some(0));
    }

    #[test]
    fn un_environnement_absent_et_un_environnement_vide_restent_differents() {
        // L'absence demande l'heritage, la table vide demande un processus sans
        // aucune variable. La demande conserve la distinction.
        let absent = demande_de_test();
        assert_eq!(absent.environnement, None);

        let vide = DemandeNodePty::resolve(
            "pwsh.exe",
            &[],
            &Opts {
                name: "pwsh".to_string(),
                cols: None,
                rows: None,
                cwd: None,
                env: Some(BTreeMap::new()),
            },
            &Supplement::new(),
        );
        assert_eq!(vide.environnement, Some(BTreeMap::new()));
        assert_ne!(vide.environnement, absent.environnement);
    }

    // -- le repli de programme ----------------------------------------------

    #[test]
    fn un_nom_de_fichier_vide_tombe_sur_le_nom_des_options() {
        // Test de veracite : en JavaScript `""` est faux, donc le repli
        // s'applique. Un portage qui testerait la nullite passerait a cote.
        assert_eq!(fichier_de_lancement("", "pwsh.exe"), "pwsh.exe");
        assert_eq!(
            fichier_de_lancement("C:\\bin\\pwsh.exe", "pwsh.exe"),
            "C:\\bin\\pwsh.exe"
        );
    }

    #[test]
    fn un_nom_de_fichier_sature_d_espaces_est_un_vrai_nom() {
        // Le reste de la famille des veracites : une chaine faite d'espaces
        // n'est pas vide, donc elle ne retombe pas. Seul `""` retombe.
        assert_eq!(fichier_de_lancement(" ", "pwsh.exe"), " ");
        assert_eq!(fichier_de_lancement("0", "pwsh.exe"), "0");
    }

    #[test]
    fn les_deux_noms_absents_donnent_un_programme_vide_et_non_une_erreur() {
        // La source ne verifie rien : elle transmet les deux vides a node-pty,
        // qui echoue de son cote. Le portage ne fabrique donc pas d'erreur.
        assert_eq!(fichier_de_lancement("", ""), "");
    }

    // -- les chemins Windows -------------------------------------------------

    #[test]
    fn un_chemin_a_antislashs_passe_sans_etre_touche() {
        // Le projet ne cible que Windows : un separateur `/` ne doit jamais
        // apparaitre la ou la source n'en posait pas, et la casse du chemin ne
        // doit pas etre pliee.
        let cwd = "C:\\PROGR~1\\Projet\\Demo";
        let demande = DemandeNodePty::resolve(
            "C:\\Windows\\System32\\cmd.exe",
            &[],
            &Opts {
                name: "cmd".to_string(),
                cols: None,
                rows: None,
                cwd: Some(cwd.to_string()),
                env: None,
            },
            &Supplement::new(),
        );
        assert_eq!(demande.repertoire.as_deref(), Some(cwd));
        assert_eq!(demande.fichier, "C:\\Windows\\System32\\cmd.exe");
        assert!(
            !demande.repertoire.as_deref().unwrap_or_default().contains('/'),
            "aucun separateur POSIX ne doit etre introduit"
        );
    }

    #[test]
    fn les_arguments_sont_recopies_tels_quels_avec_les_antislashs() {
        let args = vec![
            "-NoLogo".to_string(),
            "-Command".to_string(),
            "C:\\projet\\demo\\script.ps1 -Verbose".to_string(),
        ];
        let demande = DemandeNodePty::resolve(
            "C:\\projet\\pwsh.exe",
            &args,
            &Opts::new("pwsh"),
            &Supplement::new(),
        );
        assert_eq!(demande.args, args);
    }

    // -- la delegation ------------------------------------------------------

    #[test]
    fn le_pid_du_faux_terminal_est_lisible_a_travers_l_adaptateur() {
        let adaptateur = spawn(
            "C:\\projet\\pwsh.exe",
            &[],
            &Opts::new("pwsh"),
            FauxPty::nouveau(),
        );
        assert_eq!(adaptateur.pid(), 4242);
    }

    #[test]
    fn le_pid_est_une_photographie_prise_au_demarrage() {
        // La source ecrit `pid: proc.pid` dans un objet litteral : c'est une
        // copie, pas une delegation. La valeur de dessous change ici, celle de
        // l'objet rendu ne doit pas bouger.
        let faux = FauxPty::nouveau();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), faux);
        faux.pid.set(7);
        assert_eq!(faux.pid(), 7, "l'implementation a bien change");
        assert_eq!(adaptateur.pid(), 4242, "mais la copie reste celle du depart");
    }

    #[test]
    fn la_demande_du_terminal_adapte_est_celle_du_melange_de_la_source() {
        let args = vec!["-NoLogo".to_string()];
        let adaptateur = spawn(
            "C:\\projet\\pwsh.exe",
            &args,
            &Opts::new("pwsh"),
            FauxPty::nouveau(),
        );
        let demande = adaptateur.demande();
        assert_eq!(demande.fichier, "C:\\projet\\pwsh.exe");
        assert_eq!(demande.args, args);
        assert!(demande.use_conpty_dll);
    }

    #[test]
    fn les_quatre_operations_de_terminal_sont_transmises_dans_l_ordre() {
        let faux = FauxPty::nouveau();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), faux);
        adaptateur.write("ls -la");
        adaptateur.resize(100, 40);
        adaptateur.write("pwd");
        assert_eq!(
            faux.journal(),
            vec![
                "write(ls -la)".to_string(),
                "resize(100,40)".to_string(),
                "write(pwd)".to_string()
            ]
        );
    }

    #[test]
    fn un_signal_absent_arrive_absent_et_un_nom_vide_arrive_vide() {
        // Piege `?` contre `??` : `kill()` et `kill("")` sont deux appels
        // differents en TypeScript. Un `unwrap_or_default` de portage
        // enverrait `""` dans les deux cas, et le terminal recevrait un nom de
        // signal invalide la ou il attendait son signal par defaut.
        let faux = FauxPty::nouveau();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), faux);
        adaptateur.kill(None);
        adaptateur.kill(Some(""));
        adaptateur.kill(Some("SIGKILL"));
        assert_eq!(
            faux.journal(),
            vec![
                "kill(aucun)".to_string(),
                "kill()".to_string(),
                "kill(SIGKILL)".to_string()
            ]
        );
    }

    #[test]
    fn les_ecouteurs_sont_transmis_et_l_abonnement_reste_jetable() {
        let faux = FauxPty::nouveau();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), faux);

        // L'ordre des morceaux est verifie en routant le premier vers une case
        // et tous les suivants vers l'autre, ce qui n'a besoin d'aucun compteur.
        let premier: &'static OnceLock<String> = Box::leak(Box::new(OnceLock::new()));
        let suivant: &'static OnceLock<String> = Box::leak(Box::new(OnceLock::new()));
        let poignet = adaptateur.on_data(Box::new(move |morceau: &str| {
            if premier.get().is_none() {
                let _ = premier.set(morceau.to_string());
            } else {
                let _ = suivant.set(morceau.to_string());
            }
        }));
        assert_eq!(premier.get().map(String::as_str), Some("premier morceau"));
        assert_eq!(suivant.get().map(String::as_str), Some("second morceau"));
        poignet.dispose();

        let sortie: &'static OnceLock<Exit> = Box::leak(Box::new(OnceLock::new()));
        let poignet = adaptateur.on_exit(Box::new(move |evenement: Exit| {
            let _ = sortie.set(evenement);
        }));
        assert_eq!(
            sortie.get(),
            Some(&Exit {
                exit_code: 137,
                signal: None
            })
        );
        poignet.dispose();
    }

    #[test]
    fn le_terminal_adapte_est_utilisable_comme_objet() {
        // Le contrat de `pty_pty.rs` autorise `Box<dyn Proc>`, et l'objet rendu
        // par la source est justement un `Proc` : il doit donc se comporter
        // comme un objet, pas seulement comme une valeur.
        let adaptateur: Box<dyn Proc> = Box::new(spawn(
            "pwsh.exe",
            &[],
            &Opts::new("pwsh"),
            FauxPty::nouveau(),
        ));
        assert_eq!(adaptateur.pid(), 4242);
        adaptateur.write("exit");
    }
}
