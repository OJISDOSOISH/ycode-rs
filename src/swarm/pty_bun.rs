//! Portage de `packages/core/src/pty/pty.bun.ts`.
//!
//! # Ce que contient la source
//!
//! Vingt-six lignes, dont **six** portent du comportement, et c'est tout :
//!
//! ```ts
//! import { spawn as create } from "bun-pty"
//! import type { Opts, Proc } from "./pty"
//!
//! export type { Disp, Exit, Opts, Proc } from "./pty"
//!
//! export function spawn(file: string, args: string[], opts: Opts): Proc {
//!   const pty = create(file, args, opts)
//!   return {
//!     pid: pty.pid,
//!     onData(listener) { return pty.onData(listener) },
//!     onExit(listener) { return pty.onExit(listener) },
//!     write(data) { pty.write(data) },
//!     resize(cols, rows) { pty.resize(cols, rows) },
//!     kill(signal) { pty.kill(signal) },
//!   }
//! }
//! ```
//!
//! Trois choses y sont observables, et seulement trois :
//!
//! 1. un **reexport** des quatre types du contrat ;
//! 2. l'appel `create(file, args, opts)`, ou `opts` est transmis **tel quel** ;
//! 3. l'objet rendu, qui **recopie** `pid` et **delegue** les cinq autres
//!    operations sans les modifier.
//!
//! # Une implementation ALTERNATIVE, pas un deuxieme contrat
//!
//! Le point le plus important de ce fichier : il ne declare **aucun** type du
//! contrat. `Opts`, `Proc`, `Exit` et `Disp` sont deja portes par `pty_pty.rs`,
//! qui les importe de `packages/core/src/pty/pty.ts`, et ils sont ici
//! **reimportes** :
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
//! Ce fichier est le pendant de `pty.node.ts` : l adaptateur **Bun** du
//! contrat, a cote du contrat lui-meme (`pty_pty.rs`) et du transport
//! (`pty_protocol.rs`). Voir la section « Ce qui distingue Bun de Node » pour
//! les quatre divergences de comportement entre les deux adaptateurs.
//!
//! # Pourquoi `create` n est pas porte
//!
//! `create` est l objet `spawn` du paquet `bun-pty`. L appeler cree un
//! **vrai** pseudo terminal, donc un appel systeme Windows : `CreatePseudoConsole`
//! (ConPTY) exporte par `kernel32.dll`, son `ResizePseudoConsole`, son
//! `ClosePseudoConsole`, et la paire de tubes associee qui fait la circulation
//! dans les deux sens. Aucun de ces symboles n existe dans la bibliotheque
//! standard de Rust, et la regle du portage interdit de modifier `Cargo.toml`,
//! donc d ajouter la dependance qui les exposerait. Concretement, il manque
//! aussi tout ce qui va avec : une tache de lecture bloquante, une temporisation
//! d entrees-sorties, et la conversion des octets en texte.
//!
//! Une approximation par `std::process::Command` a ete **refusee** et c est
//! delibere. `Command` ouvre des **tubes**, pas une console : le fils n aurait
//! ni entree standard interactive, ni `isatty` vrai, donc `resize` n aurait
//! **aucun effet** et `write` n ateindrait jamais un terminal. Une API qui
//! promet de redimensionner sans redimensionner est plus dangereuse qu une API
//! absente, parce qu elle laisse croire a un terminal qui n existe pas.
//!
//! L import lui-meme n est pas traduisible non plus : Rust n a pas de
//! resolution de module a l execution. Le paquet est donc **injecte** par
//! l appelant, via le trait [`BunPtyModule`], dont le nom et le specifiqueur
//! d origine sont portes par [`MODULE_BUN_PTY`]. C est la seule adaptation de
//! la signature, et c est ce qui rend la delegation testable sans processus,
//! sans tache de fond et sans terminal.
//!
//! # Ce qui distingue Bun de Node, et qui doit rester distingue
//!
//! Les deux adaptateurs diment la meme surface, et un portage qui les confond
//! casse l un des deux. Les quatre divergences, toutes verifiees ici :
//!
//! 1. **Aucune cle ajoutee aux options.** La source Node ecrit
//!    `{ ...opts, ...(win32 ? { useConptyDll: true } : {}) }`, donc elle
//!    **fusionne** et elle **rajoute** une cle. La source Bun ecrit
//!    `create(file, args, opts)` : aucune diffusion, aucune cle posee. C est le
//!    piege de recopie entre les deux fichiers jumeaux ; [`DemandeBunPty`] ne
//!    melange donc rien et [`DemandeBunPty::cles_posees`] rend une liste vide.
//! 2. **Aucune branche de plateforme.** Ni la source ni ce fichier ne
//!    mentionnent `process.platform` : il n y a donc pas un seul `#[cfg(unix)]`
//!    ici, et les formes Unix (`forkpty`, `posix_openpt`, `ptsname`) ne sont pas
//!    mentionnees, les porter serait du code mort qui ne compiles jamais.
//! 3. **Aucun repli sur `opts.name`.** La source Node laisse node-pty trancher
//!    entre `file` et `opts.name`, et se rabat sur le second si le premier est
//!    vide. La source Bun transmet `file` **tel quel**, meme vide : un nom vide
//!    n est pas remplace par `opts.name` ici. Un [`DemandeBunPty`] construit avec
//!    une chaine vide conserve cette chaine vide.
//! 4. **Aucun supplement d options.** Comme il n'y a pas de diffusion `{ ...opts }`,
//!    le trou du typage structurel que la source Node exploite pour forcer
//!    `useConptyDll` n'existe pas : `bun-pty` recoit un `Opts` declare, rien de
//!    plus. C'est pourquoi ce fichier ne declare ni `Supplement`, ni
//!    `OptionBrute`.
//!
//! # Le piege des noms de membres en camelCase
//!
//! C'est le piege principal de ce fichier, et il est **invisible a la
//! compilation** : `onData` et `onExit` s'ecrivent `on_data` et `on_exit` en
//! Rust, et rien ne signale que l'orthographe d'origine est `onData`. Aucun de
//! ces noms ne traverse une structure serialisee, donc aucun `serde` ne peut
//! les rattraper.
//!
//! Le fichier porte donc les **deux** orthographes : [`MEMBRES`] associe le nom
//! exact de la source au nom du portage, et [`nom_rust`] fait la conversion
//! dans l'autre sens. Un relecteur peut ainsi verifier l'orthographe d'origine
//! sans ouvrir le TypeScript, et un appelant qui a besoin d'interroger le
//! contrat par son nom JavaScript dispose d'une table au lieu d'une constante
//! ecrite a la main dans chaque fichier.
//!
//! Les quatre noms de **types** (`Opts`, `Proc`, `Exit`, `Disp`) sont en
//! PascalCase, comme tous les types du projet, et ne sont pas portes ici : ils
//! viennent de `pty_pty.rs`.
//!
//! # `pid` est une photographie, pas un accesseur
//!
//! `return { pid: pty.pid, ... }` **recopie** la valeur dans un objet litteral.
//! Ce n'est pas une delegation : si la valeur changeait un jour, l'objet rendu
//! garderait l ancienne. Le portage garde cette copie, capturee dans
//! [`AdaptateurBun::nouveau`], et le test
//! `le_pid_est_une_photographie_prise_au_demarrage` le verifie en changeant la
//! valeur de l'implementation **apres** la construction. C'est le seul ecart
//! entre la delegation de la source et le trait [`Proc`], qui expose `pid()`
//! comme une methode.
//!
//! # Le piege `?` contre `??`, et ou il n'est pas
//!
//! Point de rigueur, parce que la regle du projet le demande explicitement :
//! **la source ne contient ni ternaire `?` ni coalescent `??`**. Il n'y a donc
//! rien a trancher de ce cote-la, et la relecture ne doit pas en chercher un.
//!
//! Les seuls `?` du fichier sont les **marqueurs d option** du contrat importe :
//! `signal?`, `cols?`, `rows?`, `cwd?`, `env?`. Ils ne introduisent aucun calcul,
//! ils disent seulement qu'une valeur peut manquer. Ce qui compte est de ne pas
//! transformer une absence en valeur neutre en cours de route.
//!
//! Concretement, la distinction qui compte ici est celle de `kill` :
//!
//! - `kill(undefined)` demande le **signal par defaut** de l implementation ;
//! - `kill("")` envoie un **nom de signal invalide**.
//!
//! Ce sont deux appels differents. Ce fichier transmet donc l [`Option`] tel
//! quel, sans jamais ecrire `unwrap_or_default`, et le test
//! `un_signal_absent_arrive_absent_et_un_nom_vide_arrive_vide` verrouille les
//! deux cas sur l'implementation de dessous.
//!
//! Rappel du piege voisin, cote options : `cols` et `rows` absents dans [`Opts`]
//! restent absents dans [`DemandeBunPty`]. Ils ne sont jamais remplaces par `0`,
//! qui demanderait une taille nulle ; un `0` est une **valeur presente**, il ne
//! doit pas disparaitre non plus. `env` absent (heritage) et `env` vide (aucune
//! variable) restent distincts. Cette source ne melange rien, donc elle n'a aucune
//! occasion de confondre les deux, mais le test le verifie quand meme.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

// Reexport de la source : `export type { Disp, Exit, Opts, Proc } from "./pty"`.
// Ces quatre noms appartiennent a `pty_pty.rs`, ils ne sont pas redefinis ici.
pub use crate::swarm::pty_pty::{Disp, Exit, Opts, Proc};
use crate::swarm::pty_pty::{DataListener, ExitListener};

/// Specificateur exact du paquet importe par la source.
///
/// L'import de la source est `import { spawn as create } from "bun-pty"`. Ce
/// nom n'est utilise par aucun code en aval, mais il identifie l'**unique**
/// implementation qui ferait tourner ce fichier, donc il est porte pour que la
/// relecture sache de quoi le portage se declare absent.
pub const MODULE_BUN_PTY: &str = "bun-pty";

/// Nom sous lequel la fonction `spawn` du paquet est importee.
///
/// La source ecrit `import { spawn as create }`, donc l'alias vaut `create` et
/// c'est lui qui apparait dans le corps de la fonction. C'est le seul endroit
/// ou la source Bun et la source Node se distinguent aussi nettement : Node
/// importe le module en entier sous le nom `pty`, Bun n'importe qu'une seule
/// fonction, renommee.
pub const ALIAS_IMPORT: &str = "create";

/// Les six membres de l'objet rendu, dans l'ordre de la source.
///
/// Chaque entree est le couple `(nom_en_amont, nom_du_portage)`. Le nom
/// d'amont est **exact**, casse comprise : c'est la seule endroit du fichier
/// ou l'orthographe de la source est conservee, puisque le reste de l'API
/// Rust est en minuscules. Voir la note du module sur le piege des noms.
pub const MEMBRES: [(&str, &str); 6] = [
    ("pid", "pid"),
    ("onData", "on_data"),
    ("onExit", "on_exit"),
    ("write", "write"),
    ("resize", "resize"),
    ("kill", "kill"),
];

/// Le nom du portage correspondant a un membre ecrit comme dans la source.
///
/// Renvoie `None` pour un nom que la source ne porte pas, ce qui est le
/// comportement attendu d'une table de correspondance : une faute d'orthographe
/// cote amont doit **echouer**, pas trainer dans le code.
pub fn nom_rust(membre: &str) -> Option<&'static str> {
    MEMBRES
        .iter()
        .find(|(amont, _)| *amont == membre)
        .map(|(_, aval)| *aval)
}

// ---------------------------------------------------------------------------
// Le paquet importe
// ---------------------------------------------------------------------------

/// Le paquet `bun-pty` vu comme un objet ayant une seule fonction.
///
/// C'est la traduction de l'instruction d'import. Rust n'a pas de resolution de
/// module a l'execution : le paquet ne peut donc pas etre lie statiquement, il
/// est fourni par l'appelant. Une implementation reelle de ce trait est celle
/// qui appelle `CreatePseudoConsole` et qui n'existe pas dans ce crate ; une
/// implementation d'essai se contente de noter ce qu'elle recoit, ce qui suffit
/// a verifier la delegation de [`AdaptateurBun`] sans terminal.
pub trait BunPtyModule {
    /// Le pseudo terminal rendu par [`BunPtyModule::spawn`].
    type Handle: BunPty;

    /// Cree un pseudo terminal, comme `create(file, args, opts)`.
    ///
    /// Les trois parametres sont ceux de la source, dans le meme ordre, et
    /// `opts` est transmis **tel quel** : aucune valeur par defaut n'est
    /// ajoutee, aucune cle n'est posee. La raison en est donnee dans la note du
    /// module : la source Bun ne fusionne rien, contrairement a la source Node.
    fn spawn(&self, fichier: &str, args: &[String], opts: &Opts) -> Self::Handle;
}

/// Le pseudo terminal rendu par le paquet `bun-pty`.
///
/// C'est la surface de l'objet sur lequel la source travaille, notamment
/// `pty.pid`, `pty.onData`, `pty.onExit`, `pty.write`, `pty.resize` et
/// `pty.kill`. Elle a exactement la forme du trait [`Proc`] du contrat : les
/// deux sont distincts ici parce que le premier est la surface du paquet
/// importe, dont la source n'engage rien, et le second le contrat que ce crate
/// fait respecter a ses appelants.
pub trait BunPty {
    /// Identifiant du processus fils, lu au moment de la construction de
    /// l'adaptateur et non a chaque appel.
    fn pid(&self) -> u32;

    /// Abonne un ecouteur aux morceaux de sortie, comme `pty.onData`.
    fn on_data(&self, listener: DataListener) -> Box<dyn Disp>;

    /// Abonne un ecouteur a la fin du processus, comme `pty.onExit`.
    fn on_exit(&self, listener: ExitListener) -> Box<dyn Disp>;

    /// Ecrit dans l'entree du terminal, comme `pty.write`.
    fn write(&self, data: &str);

    /// Redimensionne le terminal, comme `pty.resize`.
    fn resize(&self, cols: u32, rows: u32);

    /// Tue le processus, comme `pty.kill`. `None` est l'absence, pas une chaine
    /// vide : les deux sont deux appels differents.
    fn kill(&self, signal: Option<&str>);
}

// ---------------------------------------------------------------------------
// La demande
// ---------------------------------------------------------------------------

/// Ce que la source remet a `create` : le programme, les arguments, et les
/// options **non modifiees**.
///
/// C'est la conversion des arguments de `spawn(file, args, opts)`, et c'est tout
/// le calcul de ce fichier. La structure existe parce qu'elle rend l'absence de
/// calcul **verifiable** : un portage qui ajouterait une valeur par defaut
/// laisserait passer la compilation et se verrait ici.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandeBunPty {
    /// Programme a lancer, transmis tel quel, **y compris s'il est vide** :
    /// la source Bun n'a aucun repli sur `opts.name`, contrairement a la source
    /// Node. Une chaine vide reste donc une chaine vide.
    pub fichier: String,
    /// Arguments de la ligne de commande, recopies tels quels, sans citation ni
    /// normalisation. La source ne cite rien non plus.
    pub args: Vec<String>,
    /// Colonnes, absentes pour la taille par defaut, jamais remplacees par `0`.
    pub colonnes: Option<u32>,
    /// Lignes, absentes pour la taille par defaut, jamais remplacees par `0`.
    pub lignes: Option<u32>,
    /// Repertoire de travail, absent pour heriter de celui du processus
    /// courant. Recopie sans normalisation : sur cette cible, un chemin s'ecrit
    /// avec des antislashs et le portage n'en introduit aucun.
    pub repertoire: Option<String>,
    /// Environnement du fils, absent pour heriter de celui du processus courant.
    /// Une table vide reste une table vide : l'absence et la table vide sont deux
    /// demandes differentes, comme dans `pty_pty.rs`.
    pub environnement: Option<BTreeMap<String, String>>,
}

impl DemandeBunPty {
    /// Ce que [`spawn`] passe au paquet, sans y toucher.
    ///
    /// Le programme passe en `&str` et les arguments en tranche de `String`,
    /// donc aucun des deux n'est recopie par la fonction : seule la projection
    /// [`DemandeBunPty`] en fait une copie, et c'est elle qui sert a l'inspection
    /// et aux tests, pas l'appel. Cote TypeScript, `string[]` est un tableau
    /// **mutable**, alors que `&[String]` est une tranche immuable et ne peut
    /// donc pas contenir de trou ; le type declare `string[]`, donc la
    /// difference n'est visible sur aucune valeur conforme.
    pub fn resolve(fichier: &str, args: &[String], opts: &Opts) -> Self {
        Self {
            fichier: fichier.to_string(),
            args: args.to_vec(),
            colonnes: opts.cols,
            lignes: opts.rows,
            repertoire: opts.cwd.clone(),
            environnement: opts.env.clone(),
        }
    }

    /// Les options en JSON, sous les noms exacts du contrat.
    ///
    /// Cette vue n'existe pas en amont : la source jette l'objet d'options
    /// apres l'avoir passe a `create`. Elle est portee parce que c'est elle qui
    /// prouve que la transmission est fidele, nom pour nom, et qu'aucune cle
    /// n'a ete ajoutee. Le test `les_options_vues_par_le_paquet_sont_celles_du_contrat`
    /// s'y appuie.
    ///
    /// L'ordre des cles est celui de la declaration de [`VueOptions`], et non
    /// l'ordre alphabetique d'une table : c'est ce qui rend la chaine rendue
    /// stable quelle que soit la configuration de `serde_json`. Une absence
    /// reste un `null` explicite, jamais une valeur neutre.
    pub fn options_en_json(&self) -> String {
        let vue = VueOptions {
            cols: self.colonnes,
            rows: self.lignes,
            cwd: &self.repertoire,
            env: &self.environnement,
        };
        // Une vue de quatre options ne peut pas faire echouer une serialisation.
        serde_json::to_string(&vue).expect("quatre options ne peuvent pas faire echouer serde")
    }

    /// Les cles que la source ajoute aux options de l'appelant.
    ///
    /// C'est **la liste vide**, et c'est le comportement le plus important de
    /// ce fichier. La source Node ajoute `useConptyDll` par-dessus les options
    /// de l'appelant ; la source Bun n'ajoute rien du tout. Cette fonction est
    /// le point ou les deux adaptateurs cessent d'etre interchangeables, et le
    /// test `la_source_bun_n_ajoute_aucune_option_a_celles_de_l_appelant` la
    /// verifie.
    pub fn cles_posees() -> &'static [&'static str] {
        &[]
    }
}

/// Vue JSON des quatre options optionnelles, employees par
/// [`DemandeBunPty::options_en_json`].
///
/// Elle est privee et porte **le nom des cles du contrat**, donc l'ordre rendu
/// est celui de la declaration : c'est la seule facon d'obtenir une chaine
/// stable sans dependre de la feature `preserve_order` de `serde_json`.
///
/// Ce n'est pas une deuxieme definition du contrat : `Opts`, dans
/// `pty_pty.rs`, porte aussi `name`, qui n'a rien a faire dans cette vue puisque
/// la source Bun ne s'en sert pas.
#[derive(Serialize)]
struct VueOptions<'a> {
    cols: Option<u32>,
    rows: Option<u32>,
    cwd: &'a Option<String>,
    env: &'a Option<BTreeMap<String, String>>,
}

// ---------------------------------------------------------------------------
// L'adaptateur
// ---------------------------------------------------------------------------

/// L'objet rendu par [`spawn`] : le `Proc` de la source.
///
/// Il ne fait que deux choses : **recopier** le `pid` au moment de la
/// construction, puis **deleguer** les cinq autres operations a l'implementation
/// injectee. Aucun etat n'est ajoute, aucune operation n'est interpretee.
pub struct AdaptateurBun<P: BunPty> {
    /// Valeur de `pid` au moment de la construction, et non au moment de
    /// l'appel : la source la recopie dans un objet litteral.
    pid: u32,
    /// Ce que la source a passe a `create`, tel quel.
    demande: DemandeBunPty,
    /// Le pseudo terminal du paquet importe, jamais enveloppe ni interprete.
    interne: P,
}

impl<P: BunPty> AdaptateurBun<P> {
    /// Enveloppe une implementation, en photographiant son `pid`.
    ///
    /// L'ordre compte : le `pid` est lu **avant** que l'implementation ne soit
    /// deplacee dans la structure, et l'objet litteral de la source fait de
    /// meme.
    pub fn nouveau(interne: P, demande: DemandeBunPty) -> Self {
        let pid = interne.pid();
        Self {
            pid,
            demande,
            interne,
        }
    }

    /// La demande remise au paquet, telle que la source l'a transmise.
    ///
    /// Cette inspection n'existe pas en amont : la source jette l'objet
    /// d'options apres l'avoir passe a `create` et ne rend que le `Proc`. Elle
    /// est portee parce qu'elle est le seul endroit ou l'absence de
    /// transformation est observable.
    pub fn demande(&self) -> &DemandeBunPty {
        &self.demande
    }

    /// L'implementation sous-jacente, pour un appelant qui aurait besoin de
    /// parler au paquet directement.
    pub fn implementation(&self) -> &P {
        &self.interne
    }
}

impl<P: BunPty> fmt::Debug for AdaptateurBun<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdaptateurBun")
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
impl<P: BunPty> Proc for AdaptateurBun<P> {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn on_data(&self, listener: DataListener) -> Box<dyn Disp> {
        self.interne.on_data(listener)
    }

    fn on_exit(&self, listener: ExitListener) -> Box<dyn Disp> {
        self.interne.on_exit(listener)
    }

    /// L'objet litteral de la source ecrit `write(data) { pty.write(data) }`,
    /// **sans** `return` : la valeur rendue par le paquet, si la bibliotheque en
    /// rendait une, est donc perdue. Le trait [`Proc`] rend `()`, ce qui rend
    /// cette perte invisible : le comportement est identique, et l absence de
    /// `return` est donc documentee plutot que traduite.
    fn write(&self, data: &str) {
        self.interne.write(data);
    }

    /// Meme observation que pour [`Proc::write`] : la source ecrit le corps sans
    /// `return`.
    fn resize(&self, cols: u32, rows: u32) {
        self.interne.resize(cols, rows);
    }

    /// L'absence est transmise **comme absence**.
    ///
    /// `signal` ne traverse aucun `unwrap_or_default` : `None` reste `None` et
    /// `Some("")` reste une chaine vide.
    fn kill(&self, signal: Option<&str>) {
        self.interne.kill(signal);
    }
}

/// Demarre un pseudo terminal Bun, comme `spawn(file, args, opts)`.
///
/// # Ce qui change par rapport a la source
///
/// La source cree le pseudo terminal, puis l'enveloppe. Ici la creation est
/// **injectee** : le paquet `bun-pty` est le dernier parametre. C'est la seule
/// adaptation de la signature, et elle est inevitable : `CreatePseudoConsole`
/// n'est pas dans la bibliotheque standard et `Cargo.toml` n'est pas
/// modifiable. Voir la note du module pour le refus explicite d'une
/// approximation par `std::process::Command`.
///
/// L'enveloppe, elle, est portee telle quelle : meme `pid` photographie, memes
/// cinq delegations, et surtout **aucune** option ajoutee.
///
/// # Exemple
///
/// ```
/// use ycode::swarm::pty_bun::DemandeBunPty;
/// use ycode::swarm::pty_pty::Opts;
///
/// let demande = DemandeBunPty::resolve(
///     "C:\\projet\\pwsh.exe",
///     &["-NoLogo".to_string()],
///     &Opts::new("pwsh"),
/// );
/// assert_eq!(demande.fichier, "C:\\projet\\pwsh.exe");
/// assert_eq!(demande.args, vec!["-NoLogo".to_string()]);
/// assert_eq!(demande.colonnes, None, "absent ne veut pas dire zero");
/// assert!(DemandeBunPty::cles_posees().is_empty(), "la source n'ajoute aucune cle");
/// ```
pub fn spawn<M: BunPtyModule>(
    fichier: &str,
    args: &[String],
    opts: &Opts,
    module: &M,
) -> AdaptateurBun<M::Handle> {
    let demande = DemandeBunPty::resolve(fichier, args, opts);
    // La source ecrit `const pty = create(file, args, opts)`. L'identifiant
    // local s'appelle `pty` dans la source, et la valeur qu'il recoit est celle
    // du paquet : l'appel se fait donc sur le paquet et rien d'autre.
    let pty = module.spawn(fichier, args, opts);
    AdaptateurBun::nouveau(pty, demande)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;
    use std::rc::Rc;
    use std::sync::OnceLock;

    // -- doubles d'essai ----------------------------------------------------
    //
    // Aucune concession a la concurrence n'est faite ici : ni tache de fond,
    // ni `Mutex`, ni `Condvar`, ni temporisation, et surtout aucun processus
    // n'est cree et aucun shell n'est lance. Le journal du faux terminal est un
    // `RefCell`, qui ne peut pas bloquer et ne bloque jamais puisque chaque test
    // est mono tache et qu'aucun appel d'ecouteur n'imbrique un emprunt.
    //
    // Les ecouteurs, eux, sont contraints par le contrat : `DataListener` est un
    // `Box<dyn FnMut(&str) + Send + 'static>`. Un `RefCell` local ne peut donc
    // pas etre capture, il ne vit pas assez longtemps et n'est pas partageable.
    // D'ou les `OnceLock` installes sur une adresse `'static` par `Box::leak`.

    /// Abonnement jetable, sans etat : la source renvoie l'objet du paquet, dont
    /// le retrait n'est pas observe par ce fichier.
    struct AbonnementFaux;

    impl Disp for AbonnementFaux {
        fn dispose(&self) {}
    }

    /// Terminal d'essai : il ne simule ni terminal ni processus, il note les
    /// appels et distribue immediatement les ecouteurs, parce qu'il n'y a rien
    /// derriere.
    #[derive(Clone)]
    struct FauxPty {
        journal: Rc<RefCell<Vec<String>>>,
        pid: Rc<Cell<u32>>,
    }

    impl FauxPty {
        fn nouveau() -> Self {
            Self {
                journal: Rc::new(RefCell::new(Vec::new())),
                pid: Rc::new(Cell::new(4242)),
            }
        }

        fn journal(&self) -> Vec<String> {
            self.journal.borrow().clone()
        }
    }

    impl BunPty for FauxPty {
        fn pid(&self) -> u32 {
            self.pid.get()
        }

        fn on_data(&self, mut listener: DataListener) -> Box<dyn Disp> {
            listener("premier morceau");
            listener("second morceau");
            Box::new(AbonnementFaux)
        }

        fn on_exit(&self, mut listener: ExitListener) -> Box<dyn Disp> {
            listener(Exit {
                exit_code: 137,
                signal: Some(crate::swarm::pty_pty::Signal::Nombre(9)),
            });
            Box::new(AbonnementFaux)
        }

        fn write(&self, data: &str) {
            self.journal.borrow_mut().push(format!("write({})", data));
        }

        fn resize(&self, cols: u32, rows: u32) {
            self.journal
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
            self.journal.borrow_mut().push(entree);
        }
    }

    /// Paquet d'essai : il note la demande qu'il recoit, et rend un terminal
    /// d'essai deja construit. Il ne cree aucun processus.
    struct FauxModule {
        recu: RefCell<Option<DemandeBunPty>>,
        handle: FauxPty,
    }

    impl FauxModule {
        fn nouveau(handle: FauxPty) -> Self {
            Self {
                recu: RefCell::new(None),
                handle,
            }
        }

        fn demande_recue(&self) -> DemandeBunPty {
            self.recu
                .borrow()
                .clone()
                .expect("le paquet a recu une demande, spawn() l appelle une fois")
        }
    }

    impl BunPtyModule for FauxModule {
        type Handle = FauxPty;

        fn spawn(&self, fichier: &str, args: &[String], opts: &Opts) -> FauxPty {
            *self.recu.borrow_mut() = Some(DemandeBunPty::resolve(fichier, args, opts));
            self.handle.clone()
        }
    }

    /// Un terminal d'essai et le paquet qui le rend, prets a l'emploi.
    fn module_de_test() -> (FauxPty, FauxModule) {
        let faux = FauxPty::nouveau();
        let module = FauxModule::nouveau(faux.clone());
        (faux, module)
    }

    // -- le reexport --------------------------------------------------------

    #[test]
    fn les_quatre_types_reexportes_sont_ceux_du_contrat_et_non_des_copies() {
        // La preuve est statique : si ce fichier reddefinissai `Opts` ou `Proc`,
        // la fonction ci-dessous refuserait de compiler. C'est tout l'interet du
        // reexport, et c'est invisible a la relecture d'un simple diff.
        fn preuve(_abonnement: &dyn Disp, _sortie: Exit, _opts: Opts, _proc: &dyn Proc) {}

        let (_faux, module) = module_de_test();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), &module);
        preuve(
            &AbonnementFaux,
            Exit {
                exit_code: 0,
                signal: None,
            },
            Opts::new("pwsh"),
            &adaptateur,
        );
    }

    // -- les constantes ------------------------------------------------------

    #[test]
    fn le_paquet_importe_est_bien_bun_pty() {
        assert_eq!(MODULE_BUN_PTY, "bun-pty");
    }

    #[test]
    fn la_fonction_importee_est_bien_aliasee_sur_create() {
        // L'import de la source est `import { spawn as create }`, donc le nom
        // employe dans le corps de la fonction est `create`, pas `spawn`. C'est
        // le seul renommage du fichier, et l'oublier ferait croire que Bun
        // importe le module entier comme le fait Node.
        assert_eq!(ALIAS_IMPORT, "create");
    }

    // -- le piege des noms ---------------------------------------------------

    #[test]
    fn les_deux_noms_de_membres_camelcase_sont_bien_orthographies() {
        // Piege des noms : la source ecrit `onData` et `onExit`. En minuscules,
        // ces deux noms passeraient la compilation, car ils ne traversent aucune
        // structure serialisee : seul un regard sur la source les rattrape.
        assert_eq!(nom_rust("onData"), Some("on_data"));
        assert_eq!(nom_rust("onExit"), Some("on_exit"));
    }

    #[test]
    fn les_membres_sans_majuscule_en_amont_ont_le_meme_nom_en_portage() {
        for (amont, aval) in [
            ("pid", "pid"),
            ("write", "write"),
            ("resize", "resize"),
            ("kill", "kill"),
        ] {
            assert_eq!(nom_rust(amont), Some(aval), "membre {amont}");
        }
    }

    #[test]
    fn la_table_des_membres_couvre_les_six_operations_de_la_source() {
        // Six et non cinq : `pid` est bien un membre de l'objet litteral, aux
        // cotes des cinq methodes. L'oublier ferait croire que le `pid` est
        // delegue comme les autres, alors qu'il est photographie.
        assert_eq!(MEMBRES.len(), 6);
        let amont: Vec<&str> = MEMBRES.iter().map(|(nom, _)| *nom).collect();
        assert_eq!(amont, vec!["pid", "onData", "onExit", "write", "resize", "kill"]);
    }

    #[test]
    fn un_membre_qui_n_existe_pas_dans_la_source_ne_donne_aucun_nom() {
        // La table doit echouer sur une faute, pas tolerer un nom invente qui
        // passerait ensuite pour un membre legitime.
        assert_eq!(nom_rust("onDataa"), None);
        assert_eq!(nom_rust("ondata"), None, "la casse compte");
        assert_eq!(nom_rust("on_data"), None, "le nom du portage n'est pas une entree");
    }

    // -- la transmission des options ----------------------------------------

    #[test]
    fn la_source_bun_n_ajoute_aucune_option_a_celles_de_l_appelant() {
        // Le piege de recopie entre les deux fichiers jumeaux : la source Node
        // ajoute `useConptyDll` par-dessus les options de l'appelant, la source
        // Bun n'ajoute rien. Une liste unique de cles posee rend la difference
        // visible au lieu de la laisser deduire.
        assert!(DemandeBunPty::cles_posees().is_empty());
    }

    #[test]
    fn les_options_arrivent_chez_le_paquet_sans_etre_modifiees() {
        let (_faux, module) = module_de_test();
        let args = vec!["-NoLogo".to_string()];
        let opts = Opts {
            name: "pwsh".to_string(),
            cols: Some(120),
            rows: Some(40),
            cwd: Some("C:\\projet\\demo".to_string()),
            env: Some(BTreeMap::from([("TERM".to_string(), "xterm-256color".to_string())])),
        };
        let _ = spawn("pwsh.exe", &args, &opts, &module);
        let recu = module.demande_recue();
        assert_eq!(recu.fichier, "pwsh.exe");
        assert_eq!(recu.args, args);
        assert_eq!(recu.colonnes, Some(120));
        assert_eq!(recu.lignes, Some(40));
        assert_eq!(recu.repertoire.as_deref(), Some("C:\\projet\\demo"));
        // La projection portee et la demande reellement recue sont le meme objet
        // : c'est ce qui prouve que l'appel ne transforme rien en chemin.
        assert_eq!(recu, DemandeBunPty::resolve("pwsh.exe", &args, &opts));
    }

    #[test]
    fn des_dimensions_absentes_restent_absentes_et_un_zero_reste_un_zero() {
        // Piege `?` contre `??` : absent ne veut pas dire zero, et zero est une
        // valeur presente qui ne doit pas disparaitre. La source ne melange rien,
        // donc elle n'a aucune occasion de confondre les deux, mais le portage
        // ne doit pas le faire non plus.
        let sans = DemandeBunPty::resolve("pwsh.exe", &[], &Opts::new("pwsh"));
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
        let zero = DemandeBunPty::resolve("pwsh.exe", &[], &avec_zero);
        assert_eq!(zero.colonnes, Some(0), "zero demande une taille nulle");
        assert_eq!(zero.lignes, Some(0));
    }

    #[test]
    fn un_environnement_absent_et_un_environnement_vide_restent_differents() {
        // L'absence demande l'heritage, la table vide demande un processus sans
        // aucune variable. La source ne completant rien, elle ne peut pas
        // confondre les deux, mais la projection doit les garder distinctes.
        let absent = DemandeBunPty::resolve("pwsh.exe", &[], &Opts::new("pwsh"));
        assert_eq!(absent.environnement, None);

        let avec_vide = Opts {
            name: "pwsh".to_string(),
            cols: None,
            rows: None,
            cwd: None,
            env: Some(BTreeMap::new()),
        };
        let vide = DemandeBunPty::resolve("pwsh.exe", &[], &avec_vide);
        assert_eq!(vide.environnement, Some(BTreeMap::new()));
        assert_ne!(vide.environnement, absent.environnement);
    }

    #[test]
    fn un_programme_vide_n_est_pas_remplace_par_le_nom_des_options() {
        // La divergence la plus nette avec la source Node : celle-ci se rabat sur
        // `opts.name` quand `file` est vide, Bun ne le fait pas. Un portage qui
        // recopierait le repli de Node modifierait le comportement du fichier.
        let vide = DemandeBunPty::resolve("", &[], &Opts::new("pwsh.exe"));
        assert_eq!(vide.fichier, "", "la source Bun transmet file tel quel");
        assert_ne!(vide.fichier, "pwsh.exe");
    }

    #[test]
    fn les_options_vues_par_le_paquet_sont_celles_du_contrat() {
        // Vue de controle : les quatre noms de cles sont ceux de `pty_pty.rs`, en
        // minuscules. Une faute passerait la compilation, puisque la vue est
        // construite a la main et non derivee du contrat.
        let demande = DemandeBunPty::resolve("pwsh.exe", &[], &Opts::new("pwsh"));
        assert_eq!(demande.options_en_json(), r#"{"cols":null,"rows":null,"cwd":null,"env":null}"#);

        let avec_zero = Opts {
            name: "pwsh".to_string(),
            cols: Some(0),
            rows: Some(24),
            cwd: Some("C:\\ici".to_string()),
            env: Some(BTreeMap::new()),
        };
        let vue = DemandeBunPty::resolve("pwsh.exe", &[], &avec_zero).options_en_json();
        assert!(vue.contains(r#""cols":0"#), "zero reste une valeur presente : {vue}");
        assert!(!vue.contains("useConptyDll"), "aucune cle ajoutee : {vue}");
    }

    #[test]
    fn les_arguments_sont_recopies_tels_quels_avec_les_antislashs() {
        // Le projet ne cible que Windows : un separateur `/` ne doit pas
        // apparaitre la ou la source n'en posait pas, et la casse du chemin ne
        // doit pas etre pliee.
        let args = vec![
            "-NoLogo".to_string(),
            "-Command".to_string(),
            "C:\\projet\\demo\\script.ps1 -Verbose".to_string(),
        ];
        let demande = DemandeBunPty::resolve("C:\\Windows\\System32\\cmd.exe", &args, &Opts::new("cmd"));
        assert_eq!(demande.args, args);
        assert_eq!(demande.fichier, "C:\\Windows\\System32\\cmd.exe");
        assert!(
            !demande.fichier.contains('/'),
            "aucun separateur POSIX ne doit etre introduit"
        );
    }

    // -- la delegation ------------------------------------------------------

    #[test]
    fn le_pid_du_terminal_adapte_est_lisible_a_travers_le_portage() {
        let (_faux, module) = module_de_test();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), &module);
        assert_eq!(adaptateur.pid(), 4242);
    }

    #[test]
    fn le_pid_est_une_photographie_prise_au_demarrage() {
        // La source ecrit `pid: pty.pid` dans un objet litteral : c'est une
        // copie, pas une delegation. La valeur de dessous change ici, celle de
        // l'objet rendu ne doit pas bouger.
        let (faux, module) = module_de_test();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), &module);
        faux.pid.set(7);
        assert_eq!(faux.pid(), 7, "l'implementation a bien change");
        assert_eq!(adaptateur.pid(), 4242, "mais la copie reste celle du depart");
    }

    #[test]
    fn les_operations_de_terminal_sont_transmises_dans_l_ordre() {
        let (faux, module) = module_de_test();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), &module);
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
        // differents en TypeScript. Un `unwrap_or_default` de portage enverrait
        // `""` dans les deux cas, et le terminal recevrait un nom de signal
        // invalide la ou il attendait son signal par defaut.
        let (faux, module) = module_de_test();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), &module);
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
        let (_faux, module) = module_de_test();
        let adaptateur = spawn("pwsh.exe", &[], &Opts::new("pwsh"), &module);

        // L'ordre des morceaux est verifie en routant le premier vers une case et
        // tous les suivants vers l'autre, ce qui n'a besoin d'aucun compteur.
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
                signal: Some(crate::swarm::pty_pty::Signal::Nombre(9)),
            })
        );
        poignet.dispose();
    }

    #[test]
    fn le_terminal_adapte_est_utilisable_comme_objet() {
        // Le contrat de `pty_pty.rs` autorise `Box<dyn Proc>`, et l'objet rendu
        // par la source est justement un `Proc` : il doit donc se comporter comme
        // un objet, pas seulement comme une valeur.
        let (faux, module) = module_de_test();
        let adaptateur: Box<dyn Proc> = Box::new(spawn("pwsh.exe", &[], &Opts::new("pwsh"), &module));
        assert_eq!(adaptateur.pid(), 4242);
        adaptateur.write("exit");
        assert_eq!(faux.journal(), vec!["write(exit)".to_string()]);
    }

    #[test]
    fn la_demande_de_l_adaptateur_est_celle_transmise_au_paquet() {
        let (_faux, module) = module_de_test();
        let args = vec!["-NoLogo".to_string()];
        let adaptateur = spawn("C:\\projet\\pwsh.exe", &args, &Opts::new("pwsh"), &module);
        assert_eq!(adaptateur.demande(), &module.demande_recue());
        assert_eq!(adaptateur.demande().fichier, "C:\\projet\\pwsh.exe");
        assert_eq!(adaptateur.demande().args, args);
    }
}
