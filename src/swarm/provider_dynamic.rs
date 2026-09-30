//! Portage Rust de `opencode/packages/core/src/plugin/provider/dynamic.ts`.
//!
//! ## Ce que fait la source
//!
//! Trente et une lignes, un seul export : `DynamicProviderPlugin`. Ce n'est pas
//! un plugin de fournisseur comme les dix-neuf autres du lot, c'est le
//! **DERNIER filet** du tableau. `plugin/provider.ts:70` l'enregistre en
//! derniere position, apres les trente-deux autres, et son unique crochet n'a
//! aucune condition de reconnaissance :
//!
//! ```ts
//! if (evt.sdk) return                                     // 1. cede la place
//! const installedPath = evt.package.startsWith("file://")
//!   ? evt.package
//!   : (yield* npm.add(evt.package).pipe(Effect.orDie)).entrypoint   // 2. resout
//! if (!installedPath) throw new Error(...)                 // 3. refuse le vide
//! const mod = yield* Effect.promise(() => import(...))     // 4. importe
//! const match = Object.keys(mod).find((n) => n.startsWith("create"))  // 5. cherche
//! if (!match) throw new Error(...)                         // 6. refuse l'absence
//! evt.sdk = mod[match](evt.options)                        // 7. construit
//! ```
//!
//! Concretement : **tout** paquet AI SDK que les trente-deux autres plugins ont
//! laisse de cote passe ici, qu'il soit publie sur npm ou qu'il soit une URL
//! `file://` locale. Le plugin ne reconnait aucun nom, il ne fait que
//! fabriquer.
//!
//! ## Les types sont IMPORTES, pas redeclares
//!
//! La source ne declare aucun type. Elle manipule l'evenement du crochet
//! `aisdk.sdk`, dont la forme est dans `packages/plugin/src/v2/effect/aisdk.ts` :
//!
//! ```ts
//! sdk: {
//!   readonly model: ModelV2Info
//!   readonly package: string
//!   readonly options: Record<string, any>
//!   sdk?: any
//! }
//! ```
//!
//! Les dix-huit autres plugins `provider/*.ts` du lot ont tous recopie cette
//! forme, ce qui a produit huit `SdkEvent` divergents. Ce fichier n'en cree pas
//! un dixieme : il importe `Options` et `SdkEvent` de
//! [`provider_venice`](super::provider_venice), le module voisin dont la
//! structure est la plus proche : meme crochet `ctx.aisdk.sdk`, meme `import()`
//! dynamique, meme fabrique appelee sur `evt.options`, et un `sdk` type
//! `Option<Value>` qui accueille exactement le retour de la fabrique.
//!
//! Le meme procede est deja pose par `plugin_provider_llmgateway`, qui importe
//! ses formes de fournisseur depuis `provider_zenmux`. C'est un precedent, pas
//! une normalisation : voir la section "A dedupliquer" plus bas.
//!
//! Ce que ce fichier declare, en revanche, n'existe nulle part ailleurs dans le
//! lot : le service npm (dont `npm.ts`, 277 lignes, n'est claims par personne),
//! le module importe vu comme une liste **ordonnee** d'exports, et les trois
//! erreurs du plugin.
//!
//! ## Piege 1 : le tag npm est en MAJUSCULES
//!
//! `npm.ts:18` ecrit :
//!
//! ```ts
//! export class InstallFailedError extends Schema.TaggedErrorClass<InstallFailedError>()(
//!   "NpmInstallFailedError", { ... })
//! ```
//!
//! `"NpmInstallFailedError"` porte quatre majuscules internes. Comme
//! `Effect.orDie` transforme l'echec en **defaut** fatal, cette erreur ne
//! traverse aucune frontiere de donnees : elle ne sera jamais dans un JSON. Elle
//! reste neanmoins dans le canal d'erreur, donc visible dans les journaux, avec
//! cette casse-la. Le portage la conserve par une constante
//! [`TAG_ECHEC_INSTALLATION`] et une methode `tag()`, sans pretendre qu'elle
//! est serialisee : meme arbitrage que `tool_tools.rs`.
//!
//! Attention, le nom du tag n'est pas un nom de champ. Les champs sont `add`
//! (tableau de chaines, optionnel), `dir` (chaine) et `cause` (un
//! `Schema.Defect()`). Les trois sont en minuscules, donc aucun
//! `#[serde(rename)]`. Le portage ne derive que `Serialize` : un `Defect` n'est
//! pas une donnee JSON et ne se relit pas.
//!
//! ## Piege 2 : `!installedPath` est un test de VERACITE, pas de NULLITE
//!
//! C'est le piege central de ce fichier, et il est invisible dans une
//! signature Rust :
//!
//! ```ts
//! if (!installedPath) throw new Error(`Package ${evt.package} has no import entrypoint`)
//! ```
//!
//! `installedPath` est un `string | undefined`. Le `!` teste la **veracite**,
//! donc les deux cas suivants echouent :
//!
//! - `undefined` est falsy, donc erreur ;
//! - `""` est falsy aussi, la chaine vide etant falsy en JavaScript, donc
//!   erreur egalement.
//!
//! Un portage par `is_none()` -- le reflexe naturel de `Option<String>` --
//! laisserait passer `Some("")` et produirait une URL `file:///` a partir d'une
//! chaine vide. Les deux messages d'erreur du plugin sont donc reproduits
//! **verbatim** en anglais, et un test verrouille le cas `Some("")`.
//!
//! Le meme raisonnement vaut pour `if (!match)`, avec une difference
//! importante : la, `!match` et "aucun export trouve" **coincident**, parce
//! que `Object.keys(...).find(n => n.startsWith("create"))` ne peut renvoyer
//! que `undefined` ou une chaine non vide, la chaine vide ne commence pas par
//! `"create"`. Un test le demontre en passant `""` dans la liste des exports.
//!
//! ## Piege 3 : le ternaire de la ligne 14 n'est PAS un test de veracite
//!
//! ```ts
//! evt.package.startsWith("file://") ? evt.package : npm.add(...)
//! ```
//!
//! Ici le `?` est un ternaire **ordinaire**, dont la condition est le retour
//! booleen de `startsWith`. Ce n'est une question de nullite ni de veracite du
//! nom de paquet. Concretement : un nom de paquet **vide** ne commence pas par
//! `"file://"`, donc il part chez `npm.add("")`. Il n'est pas ecarte. C'est
//! l'inverse exact des dix-huit autres plugins, qui ecartaient le nom vide par
//! une inegalite stricte sur `@ai-sdk/xxx` : ici il n'y a aucune inegalite
//! stricte sur le nom, il n'y a qu'un prefixe.
//!
//! Le second ternaire, ligne 21, repond au meme prefixe mais sur
//! `installedPath` : une chaine deja en `file://` est importee telle quelle,
//! un chemin de disque est converti par `pathToFileURL`.
//!
//! ## Piege 4 : `Object.keys(...).find(...)` est ORDONNE
//!
//! `Object.keys` renvoie les cles d'un objet dans **l'ordre d'insertion**, et
//! `find` renvoie la **premiere** qui convient. Un module qui exporte `createFoo`
//! et `createBar` voit donc `createFoo` choisi. Un `BTreeMap` dedupliquerait les
//! noms par ordre lexicographique et pourrait choisir `createBar` : l'ordre de
//! la source est donc reproduit par un `Vec<String>`, jamais par une table. Deux
//! tests opposent les deux ordres.
//!
//! ## Ce qui n'est pas portable, et pourquoi
//!
//! - **L'`import()` dynamique.** Un paquet npm est du JavaScript, et
//!   `Cargo.toml` ne declare aucun SDK AI ; la regle du lot interdit d'y
//!   toucher. Le module importe est donc fourni par un chargeur injecte, comme
//!   partout ailleurs dans ce portage. Ce fichier ne porte que la logique : le
//!   test des prefixes, le choix de la fabrique, l'appel, l'ecriture.
//!
//! - **`pathToFileURL` est portee, mais son jeu de percent-encodage est
//!   deduit.** La fonction est pure, sans disque ni reseau, donc elle se porte.
//!   En revanche Node n'est pas installe sur ce poste : l'ensemble exact de ce
//!   que `pathToFileURL` percent-encode dans un chemin n'est pas consultable.
//!   Le portage encode l'ensemble documente dans [`caractere_echappe`] ; c'est
//!   le point le moins sur de ce fichier, et le test correspondant fixe le
//!   comportement **retenu**, pas un comportement verifie.
//!
//! - **Pas de resolution de `.` et `..`.** Node applique `path.resolve`, qui les
//!   replie. Ici le chemin vient de `npm.add(...).entrypoint`, donc il est deja
//!   resolu, et l'operation n'a jamais lieu. Elle n'est pas non plus simulee :
//!   l'ajouter serait inventer un comportement sur un cas inatteignable.
//!
//! - **`Npm.Service` n'est pas declare en entier.** `npm.ts` expose `add`,
//!   `install` et `which`, et 277 lignes de resolution. Ce plugin n'appelle que
//!   `add`, et n'en lit que le champ `entrypoint` de `Npm.EntryPoint`. Le trait
//!   [`Npm`] est donc une **projection** de l'interface d'origine. Si `npm.ts`
//!   est porte un jour, ce trait doit disparaitre au profit du vrai service :
//!   c'est le fichier de ce plugin qui doit ceder, comme `provider_zenmux` pour
//!   `plugin_provider_llmgateway`.
//!
//! ## Un ecart assume par rapport aux huit `SdkEvent` voisins
//!
//! Porter `if (evt.sdk) return` par `event.sdk.is_some()` -- ce que font les
//! modules voisins -- est **faux** dans un cas : c'est un test de veracite sur
//! une valeur JavaScript, pas une question de nullite. Un crochet enregistre
//! avant celui-ci qui aurait ecrit `evt.sdk = 0`, `""`, `false` ou `null` voit
//! sa valeur ecrasee par ce plugin, parce que ces quatre valeurs sont falsy.
//! [`valeur_est_vrai`] applique donc le vrai test de veracite JavaScript, et
//! [`crochet`] s'appuie dessus. Un test verrouille les quatre cas divergents.
//!
//! C'est une **divergence assumee** : le port le plus faithful l'emporte. Si la
//! relecture prefere l'alignement sur les huit voisins, c'est une ligne a
//! changer, et c'est le point a trancher en relecture.
//!
//! ## A dedupliquer (pour l'agent principal)
//!
//! 1. `mod.rs` declare bien `pub mod provider_dynamic;` (ligne 62, ajoutee par
//!    l'agent principal pendant le portage). Ce fichier n'a pas touche a
//!    `mod.rs`, conformement a la regle du lot.
//! 2. Les `SdkEvent` de `provider_cerebras`, `provider_cohere`,
//!    `provider_gateway`, `provider_google`, `provider_venice` et `provider_xai`,
//!    plus les `AlibabaSdkEvent` et `VercelSdkEvent` de `provider_alibaba` et
//!    `provider_vercel`, sont huit copies de la meme forme declaree par
//!    `AISDKHooks`. Ce fichier n'en ajoute pas une neuvieme. Le candidat
//!    naturel est `provider_venice`, que ce fichier a choisi comme hote ; a
//!    defaut, un module `provider_sdk_event.rs` neutre serait preferable. Ce
//!    n'est pas mon fichier a creer.
//! 3. `Npm` (ici) et l'eventuel `npm.ts` : voir plus haut.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::provider_venice::{Options, SdkEvent};

/// Identifiant du plugin, tel qu'il est enregistre par le moteur interne.
///
/// En TS : `id: "dynamic-provider"`. C'est le seul des plugins du lot dont
/// l'identifiant n'est pas le nom de son fichier : le fichier s'appelle
/// `dynamic.ts` et le plugin s'appelle `dynamic-provider`. Le tiret fait partie
/// de la donnee, il ne doit jamais devenir un soulin.
pub const PLUGIN_ID: &str = "dynamic-provider";

/// Le prefixe qui distingue une URL de fichier d'un chemin de disque.
///
/// En TS : la chaine comparee par les deux `startsWith("file://")` de la source,
/// lignes 14 et 21. Mauvaise casse, le prefixe n'est pas reconnu.
pub const PREFIXE_FICHIER: &str = "file://";

/// Le prefixe du nom d'export qui est la fabrique de fournisseur.
///
/// En TS : `Object.keys(mod).find((name) => name.startsWith("create"))`. La
/// comparaison est un prefixe, pas une egalite : `"createOpenAI"` convient,
/// `"created"` et `"Create"` non.
pub const PREFIXE_FABRIQUE: &str = "create";

/// Tag de l'erreur d'installation npm.
///
/// En TS : `Schema.TaggedErrorClass<InstallFailedError>()("NpmInstallFailedError", ...)`.
/// Quatre majuscules internes, invisibles a la compilation Rust. L'erreur ne
/// franchit aucune frontiere JSON (elle devient un defaut par `Effect.orDie`),
/// donc le tag est porte par une constante et une methode, pas par serde.
pub const TAG_ECHEC_INSTALLATION: &str = "NpmInstallFailedError";

/// Echec d'une installation npm.
///
/// Projection de `Npm.InstallFailedError` (`npm.ts:18`). Les trois champs de la
/// source sont repris a l'identique : `add` est un tableau de chaines
/// **optionnel**, `dir` est obligatoire, `cause` est un `Schema.Defect()`
/// optionnel.
///
/// Seule `Serialize` est derivee : un `Defect` est une valeur d'erreur
/// JavaScript, pas une donnee, et une implementation Rust ne peut pas la relire
/// depuis un JSON. Une deserialisation depuis la source TypeScript n'a donc
/// aucun sens ici.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[error("echec d'installation npm dans {dir}")]
pub struct ErreurInstallation {
    /// `add: Schema.Array(Schema.String).pipe(Schema.optional)`.
    ///
    /// Les paquets demandes, presents seulement quand l'installation a echoue
    /// apres avoir recu sa liste.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<Vec<String>>,

    /// `dir: Schema.String`. Le repertoire vise, toujours present.
    pub dir: String,

    /// `cause: Schema.optional(Schema.Defect())`.
    ///
    /// Le defaut JavaScript d'origine, reduit a sa representation textuelle :
    /// c'est la seule forme qui franchit un appel entre deux langages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl ErreurInstallation {
    /// Le tag de l'erreur, avec sa casse d'origine.
    pub fn tag(&self) -> &'static str {
        TAG_ECHEC_INSTALLATION
    }
}

/// Le service npm, dans la seule portion qu'exige ce plugin.
///
/// En TS : `Npm.Interface` (`npm.ts:29`). Le contrat d'origine expose `add`,
/// `install` et `which` ; ce plugin n'appelle que `add`, et n'en lit que le champ
/// `entrypoint` de `Npm.EntryPoint`. Le champ `directory` du meme objet n'est
/// jamais lu, donc n'est pas porte.
///
/// La valeur de retour est ce champ `entrypoint`, d'ou un `Option<String>` :
/// `None` signifie que la resolution a echoue **sans lever**, ce qui est
/// different d'un `Err`. C'est `resolveEntryPoint` (`npm.ts:52`) qui produit
/// cette distinction, en avalent l'exception de resolution.
///
/// `npm.ts` n'est claims par aucun agent du lot. Si son portage arrive, ce trait
/// doit etre supprime au profit du vrai service : voir la section "Ce qui n'est
/// pas portable" dans l'entete du module.
pub trait Npm {
    /// `add(pkg)` : installe le paquet si besoin et renvoie son point d'entree.
    ///
    /// Une erreur n'est pas attrsapee par l'appelant : la source applique
    /// `Effect.orDie`, qui transforme l'echec en defaut fatal.
    fn add(&self, paquet: &str) -> Result<Option<String>, ErreurInstallation>;
}

/// La fabrique qui construit le SDK, une fois le bon export choisi.
///
/// En TS : `mod[match](evt.options)`, c'est-a-dire une fonction
/// `(options: any) => any` exportee par le paquet.
///
/// Le type est `Fn(&Options) -> Value` et non `FnOnce(&Options) -> Value` pour
/// une raison technique, pas de fond : `Box<dyn FnOnce(..)>` n'est pas stable en
/// Rust. Le comportement est identique, la fabrique est **appelee exactement une
/// fois** par crochet, et c'est [`Sortie::Construit`] qui le garantit.
pub type Fabrique = Box<dyn Fn(&Options) -> Value>;

/// Le module importe, vu sous la forme que rend l'`import()` dynamique.
///
/// En TS : `mod`, type `Record<string, (options: any) => any>`.
///
/// Le champ qui compte est [`Module::noms`] : il reproduit `Object.keys(mod)`
/// **dans l'ordre d'enumeration**, parce que `find` renvoie la premiere
/// correspondance et que cet ordre est donc observable. Une table triee
/// changerait le resultat sur un module qui exporte deux fabriques.
pub struct Module {
    noms: Vec<String>,
    fabrique: Fabrique,
}

impl Module {
    /// Construit un module a partir de ses exports, dans l'ordre, et de la
    /// fabrique associee a l'export retenu.
    ///
    /// L'ordre de `noms` est celui de `Object.keys`. C'est le seul parametre
    /// qui porte une information de comportement.
    pub fn nouveau(noms: Vec<String>, fabrique: Fabrique) -> Self {
        Self { noms, fabrique }
    }

    /// `Object.keys(mod)`, dans l'ordre d'enumeration.
    pub fn noms(&self) -> &[String] {
        &self.noms
    }
}

impl std::fmt::Debug for Module {
    /// Les fabriques n'ont pas de representation textuelle, donc seule la liste
    /// des exports est affichee. C'est de toute facon ce qu'un developpeur veut
    /// lire dans un echec de test.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Module")
            .field("noms", &self.noms)
            .finish_non_exhaustive()
    }
}

/// Ce que le crochet a fait de l'evenement.
///
/// Le TypeScript distingue deux sorties et deux exceptions ; les deux exceptions
/// sont portees par [`Erreur`], donc l'enum n'a que deux variantes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sortie {
    /// `if (evt.sdk) return` : un crochet enregistre avant celui-ci a deja
    /// rempli le champ, celui-ci rend la main sans rien installer ni importer.
    Cede,
    /// `evt.sdk = mod[match](evt.options)` a ete fait.
    Construit,
}

/// Les trois exceptions que le crochet peut lever.
///
/// Les deux messages anglais sont **verbatim** depuis la source. Ce ne sont pas
/// des chaines d'acces au journal : ce sont les messages que le TypeScript
/// affiche, donc ils font partie du contrat.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Erreur {
    /// `npm.add` a echoue, remonte tel quel.
    ///
    /// En TS c'est un `Effect.orDie` : l'echec devient un defaut et remonte
    /// jusqu'a la racine, sans etre rattrape. Le `Result` de Rust est
    /// l'equivalent le plus proche qui ne fasse pas paniquer.
    #[error("Package {paquet}: installation npm impossible")]
    Installation {
        /// Le paquet que le plugin essayait d'installer.
        paquet: String,
        /// L'erreur npm d'origine, avec son tag.
        cause: ErreurInstallation,
    },

    /// `if (!installedPath) throw new Error("Package ... has no import entrypoint")`.
    #[error("Package {paquet} has no import entrypoint")]
    AucunPointEntree {
        /// Le paquet cite dans le message d'origine.
        paquet: String,
    },

    /// `if (!match) throw new Error("Package ... has no provider factory export")`.
    #[error("Package {paquet} has no provider factory export")]
    AucuneFabrique {
        /// Le paquet cite dans le message d'origine.
        paquet: String,
    },
}

/// `evt.package.startsWith("file://")`.
///
/// Un test de **prefixe**, pas d'egalite, et surtout pas un test de veracite :
/// la chaine vide ne commence pas par `"file://"`, donc elle est traitee comme
/// n'importe quel nom de paquet et part chez `npm.add("")`. C'est l'inverse des
/// dix-huit plugins voisins, qui ecartaient le nom vide par une inegalite
/// stricte sur `@ai-sdk/xxx`.
pub fn est_url_fichier(chemin: &str) -> bool {
    chemin.starts_with(PREFIXE_FICHIER)
}

/// `Object.keys(mod).find((name) => name.startsWith("create"))`.
///
/// Rend la **premiere** cle, dans l'ordre recu, qui commence par `"create"`.
/// L'ordre est donc significatif et la liste est un `Vec`, pas une table.
pub fn premier_export(noms: &[String]) -> Option<&str> {
    noms.iter()
        .map(String::as_str)
        .find(|nom| nom.starts_with(PREFIXE_FABRIQUE))
}

/// Test de veracite JavaScript, applique a une valeur JSON.
///
/// Le `if (evt.sdk) return` de la source est un test de veracite, pas une
/// question de nullite. Sur une valeur JavaScript :
///
/// - `null`, `undefined`, `false`, `0`, `NaN` et `""` sont falsy ;
/// - `[]` et `{}` sont **truthy**, meme vides ;
/// - tout le reste est truthy.
///
/// Un `Option::is_some()` n'en reproduit que la premiere ligne.
pub fn valeur_est_vrai(valeur: &Value) -> bool {
    match valeur {
        Value::Null => false,
        Value::Bool(valeur) => *valeur,
        Value::Number(nombre) => nombre.as_f64().map(|v| v != 0.0 && !v.is_nan()).unwrap_or(false),
        Value::String(texte) => !texte.is_empty(),
        // Tableaux et objets : toujours truthy en JavaScript, meme vides.
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `if (evt.sdk) return`, le test de veracite de la source.
///
/// Renvoie `true` quand un crochet enregistre avant celui-ci a deja rempli le
/// champ **et** que cette valeur est truthy. C'est la seule sortie normale du
/// crochet : le plugin est enregistre en dernier (`provider.ts:70`), il ne doit
/// donc jamais ecraser un SDK deja construit.
pub fn sdk_deja_construit(evenement: &SdkEvent) -> bool {
    match &evenement.sdk {
        None => false,
        Some(valeur) => valeur_est_vrai(valeur),
    }
}

/// `pathToFileURL(chemin).href`.
///
/// La fonction est pure, sans disque ni reseau, donc elle se porte. Trois etapes :
///
/// 1. les separateurs Windows `\` deviennent des `/`, comme le fait le parseur
///    d'URL pour un schema special ;
/// 2. un chemin UNC `//partage/...` donne son premier segment comme autorite,
///    d'ou `file://partage/...` et non `file:///partage/...` ;
/// 3. le reste est place sous `file:///`, percent-encode.
///
/// Une lettre de lecteur n'a pas besoin d'etat : `C:\\cache` normalise donne
/// `C:/cache`, qui sous `file:///` donne exactement `file:///C:/cache`. Le
/// deux-points de la lettre reste litteral, ce qui est la forme que Node produit
/// et que `import()` accepte.
///
/// **Ce qui n'est pas verifie** : l'ensemble de percent-encodage de Node.
/// `pathToFileURL` fait partie de `lib/internal/url.js`, et Node n'est pas
/// installe sur ce poste, donc le tableau de reference n'a pas pu etre lu. Ce
/// qui est implemente ici est decrit dans [`caractere_echappe`] et fixe par un
/// test, mais ce test **fixe le comportement retenu**, il ne prouve pas
/// l'egalite avec Node.
///
/// **Ce qui n'est pas implemente** : le repli des segments `.` et `..`. Node
/// applique `path.resolve` avant d'encoder, donc ces segments sont deja
/// disparus quand l'appel est reel. Les simuler ici serait inventer un
/// comportement sur un cas inatteignable.
pub fn chemin_vers_url_fichier(chemin: &str) -> String {
    let normalise: String = chemin.replace('\\', "/");

    // Etape 2 : autorite UNC. Le prefixe `//` fait deux octets ASCII, la coupe
    // par octet qui suit ne peut donc pas tomber au milieu d'un caractere.
    if let Some(reste) = normalise.strip_prefix("//") {
        let (hote, suite) = match reste.find('/') {
            Some(coupe) => (&reste[..coupe], &reste[coupe..]),
            None => (reste, "/"),
        };
        return format!(
            "file://{}{}",
            percent_encoder(hote),
            percent_encoder(suite)
        );
    }

    // Etape 3 : le chemin commence par un `/`, qui est celui qui suit
    // l'autorite vide. Un chemin POSIX absolu en a deja un, donc il ne faut pas
    // en ajouter un second -- `file:///` + `/cache` donnerait quatre barres. Une
    // lettre de lecteur n'en a pas, donc on le prefixe : `C:/cache` donne
    // `file:///C:/cache`, la forme que Node produit et que `import()` accepte.
    let chemin = if normalise.starts_with('/') {
        normalise
    } else {
        format!("/{}", normalise)
    };

    format!("file://{}", percent_encoder(&chemin))
}

/// Le percent-encodage du chemin, caractere par caractere.
fn percent_encoder(chemin: &str) -> String {
    let mut sortie = String::with_capacity(chemin.len());
    for caractere in chemin.chars() {
        if caractere_echappe(caractere) {
            encoder_un_caractere(caractere, &mut sortie);
        } else {
            sortie.push(caractere);
        }
    }
    sortie
}

/// L'ensemble de ce que le portage percent-encode dans un chemin.
///
/// Deduit, **pas verifie** : voir [`chemin_vers_url_fichier`].
///
/// - `%` est toujours echappe, sinon un `%20` deja present deviendrait `%2520` a
///   la deuxieme passe ;
/// - les controles C0 (U+0000 a U+001F) et tout ce qui est au-dessus de U+007E,
///   ce qui couvre le repertoire de cache de npm comme les chemins non latins ;
/// - les separateurs de segment et de chemin que l'URL ne peut pas porter tels
///   quels.
pub fn caractere_echappe(caractere: char) -> bool {
    let code = caractere as u32;
    code < 0x20
        || code > 0x7E
        || matches!(
            caractere,
            '%' | ' ' | '"' | '#' | '<' | '>' | '?' | '`' | '[' | ']' | '^' | '|' | '{' | '}'
        )
}

/// Ecrit un caractere en UTF-8, chaque octet en `%XX` majuscule.
fn encoder_un_caractere(caractere: char, sortie: &mut String) {
    let mut tampon = [0u8; 4];
    for octet in caractere.encode_utf8(&mut tampon).as_bytes() {
        sortie.push('%');
        sortie.push(nibble_en_hex(octet >> 4));
        sortie.push(nibble_en_hex(octet & 0x0F));
    }
}

/// Un quartets d'octet, en majuscule comme dans toutes les URL.
fn nibble_en_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + nibble - 10) as char,
    }
}

/// Le descripteur du plugin.
///
/// En TS : l'objet passe a `define`, dont `define` se contente de le renvoyer
/// (`plugin/internal.ts:59`). Le champ `effect` est une fonction, sans
/// representation : il est porte par [`Plugin::enregistrer`], qui est le crochet.
/// Il reste ici la seule donnee du descripteur, l'identifiant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plugin {
    /// `id`, tiret compris.
    pub id: String,
}

impl Plugin {
    /// Le descripteur du plugin, avec l'identifiant de la source.
    pub fn nouveau() -> Self {
        Self {
            id: PLUGIN_ID.to_string(),
        }
    }

    /// Enregistre le crochet sur `ctx.aisdk.sdk`.
    ///
    /// L'appel est la traduction de l'unique
    /// `yield* ctx.aisdk.sdk(Effect.fn(function* (evt) { ... }))`. Le `ctx` et
    /// l'effet `sdk` appartiennent au moteur de plugins, qui n'est pas porte ici :
    /// la fonction recoit donc directement l'evenement, le service npm et le
    /// chargeur.
    pub fn enregistrer<N, F>(
        &self,
        evenement: &mut SdkEvent,
        npm: &N,
        charger: F,
    ) -> Result<Sortie, Erreur>
    where
        N: Npm + ?Sized,
        F: FnOnce(&str) -> Result<Module, Erreur>,
    {
        crochet(evenement, npm, charger)
    }
}

impl Default for Plugin {
    fn default() -> Self {
        Self::nouveau()
    }
}

/// Le crochet du plugin, en version pure et testable.
///
/// `charger` est l'`import()` dynamique : il recoit l'URL -- ou la chaine
/// `file://` deja prete -- et rend le module importe. La source enveloppe cet
/// appel dans un `Effect.promise(...).pipe(Effect.orDie)`, donc toute erreur
/// qu'il rend ici est deja sous la forme d'un defaut fatally remonte.
///
/// L'ordre des sept etapes est celui de la source, et il est significatif :
/// l'evenement est teste **avant** toute installation, donc un paquet deja servi
/// n'est jamais passe chez `npm`.
pub fn crochet<N, F>(evenement: &mut SdkEvent, npm: &N, charger: F) -> Result<Sortie, Erreur>
where
    N: Npm + ?Sized,
    F: FnOnce(&str) -> Result<Module, Erreur>,
{
    // 1. `if (evt.sdk) return` : sortie normale, pas une erreur. C'est un test
    //    de veracite, voir [valeur_est_vrai].
    if sdk_deja_construit(evenement) {
        return Ok(Sortie::Cede);
    }

    // 2. Le ternaire de la ligne 14. Sa condition est le retour booleen de
    //    `startsWith`, pas une question de nullite ni de veracite : un nom de
    //    paquet vide part donc chez npm.
    let resolu = if est_url_fichier(&evenement.package) {
        Some(evenement.package.clone())
    } else {
        npm.add(&evenement.package)
            .map_err(|cause| Erreur::Installation {
                paquet: evenement.package.clone(),
                cause,
            })?
    };

    // 3. `if (!installedPath) throw` : TEST DE VERACITE sur une chaine. Les deux
    //    branches convergent ici, et dans la branche `file://` la chaine
    //    commence forcement par sept caracteres non vides, donc seul le chemin
    //    venu de npm peut declencher l'erreur. `None` et `Some("")` echouent tous
    //    les deux : un `is_none()` seul laisserait passer la chaine vide.
    let chemin = match resolu {
        Some(chemin) if !chemin.is_empty() => chemin,
        _ => {
            return Err(Erreur::AucunPointEntree {
                paquet: evenement.package.clone(),
            })
        }
    };

    // 4. Le second ternaire, ligne 21 : meme test de prefixe, sur le chemin
    //    resolu cette fois. C'est le seul endroit du fichier ou une conversion de
    //    chemin en URL a lieu.
    let url = if est_url_fichier(&chemin) {
        chemin
    } else {
        chemin_vers_url_fichier(&chemin)
    };

    let module = charger(&url)?;

    // 5. `Object.keys(mod).find((name) => name.startsWith("create"))`.
    // 6. `if (!match) throw` : ici la veracite de `match` et l'absence de
    //    correspondance coincident, car `find` ne peut renvoyer que `undefined`
    //    ou une chaine non vide.
    if premier_export(module.noms()).is_none() {
        return Err(Erreur::AucuneFabrique {
            paquet: evenement.package.clone(),
        });
    }

    // 7. `evt.sdk = mod[match](evt.options)`.
    let fabrique = &module.fabrique;
    evenement.sdk = Some(fabrique(&evenement.options));

    Ok(Sortie::Construit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    /// Journal d'appels, partage entre un test et les doubles qu'il arme.
    ///
    /// `Rc` et `RefCell` parce que ce sont des tests a un seul fil : aucun
    /// `thread`, aucun `Mutex`, aucune attente, aucune boucle, aucun acces
    /// reseau. Tout est instantane et deterministe.
    type Journal = Rc<RefCell<Vec<String>>>;

    /// Ce que le faux npm doit repondre.
    enum Reponse {
        /// `Npm.EntryPoint.entrypoint`. `None` si la resolution a echoue sans
        /// lever, `Some` si un chemin a ete resolu. La chaine vide est une
        /// reponse valide : c'est tout l'interet du test.
        Point(Option<String>),
        /// L'installation a echoue.
        Echec(ErreurInstallation),
    }

    /// Faux service npm : note les paquets demandes, puis repond.
    struct NpmFaux {
        installes: Journal,
        reponse: Reponse,
    }

    impl NpmFaux {
        fn nouveau(reponse: Reponse) -> Self {
            Self {
                installes: Rc::new(RefCell::new(Vec::new())),
                reponse,
            }
        }

        /// Faux npm qui resout toujours le meme point d'entree.
        fn qui_resout(entree: &str) -> Self {
            Self::nouveau(Reponse::Point(Some(entree.to_string())))
        }

        /// Les paquets passes a `add`, dans l'ordre.
        fn installes(&self) -> Vec<String> {
            self.installes.borrow().clone()
        }
    }

    impl Npm for NpmFaux {
        fn add(&self, paquet: &str) -> Result<Option<String>, ErreurInstallation> {
            self.installes.borrow_mut().push(paquet.to_string());
            match &self.reponse {
                Reponse::Point(point) => Ok(point.clone()),
                Reponse::Echec(erreur) => Err(erreur.clone()),
            }
        }
    }

    /// Le banc de test : un module importe et un chargeur, avec trois journaux
    /// **distincts**.
    ///
    /// Les trois journaux sont separes a dessein. Un seul journal pour les URL et
    /// les options rendrait les assertions ambigues : le test ne pourrait plus
    /// distinguer une URL importee d'un jeu d'options recu par la fabrique.
    struct Banc {
        /// Les URL passees au chargeur, donc les chemins reellement importes.
        urls: Journal,
        /// Les options vues par la fabrique, sous forme de cles jointes.
        options_vues: Journal,
        /// Le nombre d'appels a la fabrique.
        appels: Rc<Cell<usize>>,
        /// `Object.keys(mod)`, dans l'ordre d'enumeration.
        exports: Vec<String>,
    }

    impl Banc {
        fn nouveau(exports: &[&str]) -> Self {
            Self {
                urls: Rc::new(RefCell::new(Vec::new())),
                options_vues: Rc::new(RefCell::new(Vec::new())),
                appels: Rc::new(Cell::new(0)),
                exports: exports.iter().map(|nom| nom.to_string()).collect(),
            }
        }

        /// Le faux module. Une fabrique neuve a chaque appel, donc un test peut
        /// reconstruire le module autant de fois qu'il le souhaite.
        fn module(&self) -> Module {
            let appels = Rc::clone(&self.appels);
            let options_vues = Rc::clone(&self.options_vues);
            let fabrique: Fabrique = Box::new(move |options: &Options| {
                appels.set(appels.get() + 1);
                options_vues
                    .borrow_mut()
                    .push(options.keys().cloned().collect::<Vec<String>>().join(","));
                json!({ "construit": true })
            });
            Module::nouveau(self.exports.clone(), fabrique)
        }

        /// Le faux `import()`. Il note l'URL et rend le module.
        fn chargeur(&self) -> impl FnOnce(&str) -> Result<Module, Erreur> + '_ {
            let urls = Rc::clone(&self.urls);
            let module = self.module();
            move |url: &str| {
                urls.borrow_mut().push(url.to_string());
                Ok(module)
            }
        }

        fn urls(&self) -> Vec<String> {
            self.urls.borrow().clone()
        }

        fn options_vues(&self) -> Vec<String> {
            self.options_vues.borrow().clone()
        }

        fn appels(&self) -> usize {
            self.appels.get()
        }
    }

    /// Evenement d'essai, avec le paquet donne et une option unique.
    fn evenement(paquet: &str) -> SdkEvent {
        let mut options = Options::new();
        options.insert("apiKey".to_string(), json!("secret"));
        SdkEvent::new(paquet, options)
    }

    /// Evenement d'essai sans aucune option.
    fn evenement_sans_options(paquet: &str) -> SdkEvent {
        SdkEvent::new(paquet, Options::new())
    }

    /// Echec npm d'essai, avec le tag de la source.
    fn echec_npm() -> ErreurInstallation {
        ErreurInstallation {
            add: Some(vec!["@ai-sdk/casse".to_string()]),
            dir: "/cache/packages/@ai-sdk/casse".to_string(),
            cause: Some("EACCES: permission denied".to_string()),
        }
    }

    // ------------------------------------------- identifiants et constantes

    #[test]
    fn l_identifiant_du_plugin_est_celui_de_la_source_et_garde_son_tiret() {
        // Le fichier s'appelle `dynamic.ts` mais le plugin s'appelle
        // `dynamic-provider` : confondre les deux est l'erreur facile ici.
        assert_eq!(PLUGIN_ID, "dynamic-provider");
        assert_eq!(Plugin::nouveau().id, "dynamic-provider");
        assert_eq!(Plugin::nouveau(), Plugin::default());
        assert!(PLUGIN_ID.contains('-'), "le tiret fait partie de la donnee");
        assert!(!PLUGIN_ID.contains('_'), "le tiret ne doit pas devenir un soulin");
        assert_eq!(
            serde_json::to_value(Plugin::nouveau()).unwrap(),
            json!({ "id": "dynamic-provider" })
        );
    }

    #[test]
    fn les_constantes_de_la_source_sont_reprises_telles_quelles() {
        assert_eq!(PREFIXE_FICHIER, "file://");
        assert_eq!(PREFIXE_FABRIQUE, "create");
        assert_eq!(TAG_ECHEC_INSTALLATION, "NpmInstallFailedError");
    }

    // --------------------------------------- la garde de veracite, et sa portee

    #[test]
    fn un_sdk_deja_construit_cede_la_place_sans_installer_ni_importer() {
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let banc = Banc::nouveau(&["createX"]);
        let mut evt = evenement("mon-paquet");
        evt.sdk = Some(json!({ "languageModel": "objet vivant" }));

        let sortie = crochet(&mut evt, &npm, banc.chargeur());

        assert_eq!(sortie, Ok(Sortie::Cede));
        assert!(npm.installes().is_empty(), "npm ne doit pas etre appele");
        assert!(banc.urls().is_empty(), "le chargeur ne doit pas etre appele");
        assert_eq!(banc.appels(), 0, "la fabrique ne doit pas etre appelee");
        assert_eq!(evt.sdk, Some(json!({ "languageModel": "objet vivant" })));
    }

    #[test]
    fn une_valeur_falsy_dans_sdk_ne_bloque_pas_le_plugin() {
        // Piege de veracite, et divergence assumee avec les huit voisins qui
        // testent `is_some()`. En JavaScript ces quatre valeurs sont falsy, donc
        // `if (evt.sdk) return` ne s'execute pas et le plugin les ecrase.
        for valeur in [json!(null), json!(false), json!(0), json!("")] {
            assert!(!valeur_est_vrai(&valeur), "{valeur} doit etre falsy");
            let mut evt = evenement("exemple");
            evt.sdk = Some(valeur.clone());
            assert!(
                !sdk_deja_construit(&evt),
                "{valeur} est falsy, le plugin doit continuer"
            );
        }

        // Tableaux et objets, meme vides, sont truthy : le crochet cede.
        for valeur in [json!([]), json!({}), json!([1]), json!({ "a": 1 })] {
            assert!(valeur_est_vrai(&valeur), "{valeur} doit etre truthy");
            let mut evt = evenement("exemple");
            evt.sdk = Some(valeur);
            assert!(sdk_deja_construit(&evt));
        }

        // Un champ absent est la sortie normale du crochet.
        let evt = evenement("exemple");
        assert!(!sdk_deja_construit(&evt));

        // Et le plugin ecrase effectivement une valeur falsy.
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let banc = Banc::nouveau(&["createX"]);
        let mut evt = evenement("exemple");
        evt.sdk = Some(json!(0));
        let sortie = crochet(&mut evt, &npm, banc.chargeur());
        assert_eq!(sortie, Ok(Sortie::Construit));
        assert_eq!(evt.sdk, Some(json!({ "construit": true })), "le 0 est ecrase");
        assert_eq!(banc.appels(), 1);
    }

    // ----------------------------------- le ternaire, qui n'est pas un test de veracite

    #[test]
    fn un_nom_de_paquet_vide_est_installe_comme_un_autre() {
        // C'est le point de fait du fichier. `""` ne commence pas par "file://",
        // donc le ternaire part chez npm avec la chaine vide. Il n'est pas ecarte
        // comme il le serait avec une inegalite stricte sur "@ai-sdk/xxx".
        assert!(!est_url_fichier(""), "le prefixe ne correspond pas");

        let npm = NpmFaux::qui_resout("/cache/index.js");
        let banc = Banc::nouveau(&["createX"]);
        let mut evt = evenement_sans_options("");

        let sortie = crochet(&mut evt, &npm, banc.chargeur());

        assert_eq!(sortie, Ok(Sortie::Construit));
        assert_eq!(
            npm.installes(),
            vec![String::new()],
            "npm doit avoir recu la chaine vide"
        );
        assert_eq!(banc.urls(), vec!["file:///cache/index.js".to_string()]);
    }

    #[test]
    fn un_paquet_en_url_de_fichier_va_directement_a_l_import() {
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let banc = Banc::nouveau(&["createLocal"]);
        let mut evt = evenement("file:///chemin/local/module.mjs");

        let sortie = crochet(&mut evt, &npm, banc.chargeur());

        assert_eq!(sortie, Ok(Sortie::Construit));
        assert!(npm.installes().is_empty(), "aucune installation pour une URL file://");
        // L'URL est passee telle quelle : la seconde branche du ternaire ne
        // reconvertit pas une URL qui en est deja une.
        assert_eq!(banc.urls(), vec!["file:///chemin/local/module.mjs".to_string()]);
    }

    #[test]
    fn le_prefixe_file_est_teste_sur_les_son_deux_occurrences() {
        assert!(est_url_fichier("file://"));
        assert!(est_url_fichier("file:///a"));
        assert!(!est_url_fichier("file:/a"), "un seul slash ne suffit pas");
        assert!(!est_url_fichier("File:///a"), "le prefixe est sensible a la casse");
        assert!(!est_url_fichier(" file:///a"), "le prefixe ne peut pas etre decale");
        assert!(!est_url_fichier("/a/b.js"));
        assert!(!est_url_fichier(""));
        // Les deux ternaires de la source repondent au meme predicat, donc la
        // meme fonction : c'est ce qui garantit qu'une URL deja prete n'est
        // jamais reconvertie.
        assert_eq!(est_url_fichier("file:///cache/x.js"), est_url_fichier("file:///cache/x.js"));
    }

    // ------------------------------- piege 2 : le point d'entree vide est un echec

    #[test]
    fn un_point_d_entree_absent_leve_le_message_de_la_source() {
        let npm = NpmFaux::nouveau(Reponse::Point(None));
        let banc = Banc::nouveau(&["createX"]);
        let mut evt = evenement("@ai-sdk/inconnu");

        let erreur = crochet(&mut evt, &npm, banc.chargeur())
            .expect_err("un point d'entree absent doit echouer");

        assert_eq!(erreur, Erreur::AucunPointEntree { paquet: "@ai-sdk/inconnu".to_string() });
        assert_eq!(erreur.to_string(), "Package @ai-sdk/inconnu has no import entrypoint");
        assert!(banc.urls().is_empty(), "rien ne doit etre importe");
        assert!(evt.sdk.is_none());
    }

    #[test]
    fn un_point_d_entree_vide_leve_la_meme_erreur_que_son_absence() {
        // Le piege central. `if (!installedPath)` est un test de VERACITE : la
        // chaine vide est falsy en JavaScript. Un portage par `is_none()`
        // laisserait passer `Some("")` et produirait `file:///`.
        let npm = NpmFaux::nouveau(Reponse::Point(Some(String::new())));
        let banc = Banc::nouveau(&["createX"]);
        let mut evt = evenement("@ai-sdk/inconnu");

        let erreur = crochet(&mut evt, &npm, banc.chargeur())
            .expect_err("une chaine vide doit echouer comme une absence");

        assert_eq!(erreur, Erreur::AucunPointEntree { paquet: "@ai-sdk/inconnu".to_string() });
        assert_eq!(erreur.to_string(), "Package @ai-sdk/inconnu has no import entrypoint");
        assert!(banc.urls().is_empty(), "rien ne doit etre importe");
    }

    #[test]
    fn un_echec_d_installation_est_remonte_sans_etre_attrape() {
        let npm = NpmFaux::nouveau(Reponse::Echec(echec_npm()));
        let banc = Banc::nouveau(&["createX"]);
        let mut evt = evenement("@ai-sdk/casse");

        let erreur = crochet(&mut evt, &npm, banc.chargeur())
            .expect_err("un echec d'installation doit remonter");

        match &erreur {
            Erreur::Installation { paquet, cause } => {
                assert_eq!(paquet, "@ai-sdk/casse");
                assert_eq!(cause.tag(), "NpmInstallFailedError");
                assert_eq!(cause.dir, "/cache/packages/@ai-sdk/casse");
                assert_eq!(cause.add, Some(vec!["@ai-sdk/casse".to_string()]));
                assert_eq!(cause.cause.as_deref(), Some("EACCES: permission denied"));
            }
            autre => panic!("variante inattendue : {autre:?}"),
        }
        assert_eq!(npm.installes(), vec!["@ai-sdk/casse".to_string()]);
        assert!(banc.urls().is_empty());
        assert!(evt.sdk.is_none());
    }

    #[test]
    fn le_tag_de_l_erreur_npm_garde_ses_majuscules() {
        // PIEGE 1 : invisible a la compilation, visible dans les journaux.
        let erreur = echec_npm();
        assert_eq!(erreur.tag(), "NpmInstallFailedError");
        assert_eq!(TAG_ECHEC_INSTALLATION, erreur.tag());
        assert_eq!(
            TAG_ECHEC_INSTALLATION.chars().filter(|c| c.is_ascii_uppercase()).count(),
            4,
            "NpmInstallFailedError porte quatre majuscules"
        );
        assert!(!TAG_ECHEC_INSTALLATION.contains('_'), "le tag n'est pas un identifiant Rust");

        // Les champs restent en minuscules, donc aucun renommage n'est pose.
        let objet = serde_json::to_value(&erreur).unwrap();
        assert_eq!(objet["dir"], json!("/cache/packages/@ai-sdk/casse"));
        assert_eq!(objet["add"], json!(["@ai-sdk/casse"]));
        assert!(objet.get("cause").is_none(), "cause est un Defect, pas une donnee JSON");
        assert!(objet.get("tag").is_none(), "le tag n'est pas un champ serialise");
        assert!(objet.get("_tag").is_none(), "le tag ne sort pas non sous le nom interne");

        // Sans `add` ni `cause`, les cles optionnelles disparaitent au lieu de
        // valoir `null`.
        let sans_add = ErreurInstallation { add: None, dir: "/d".to_string(), cause: None };
        assert_eq!(serde_json::to_value(&sans_add).unwrap(), json!({ "dir": "/d" }));
    }

    // ------------------------------- le choix de la fabrique, qui depend de l'ordre

    #[test]
    fn la_premiere_fabrique_par_prefixe_est_choisie_dans_lordre_des_exports() {
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let banc = Banc::nouveau(&["default", "createA", "createB"]);
        let mut evt = evenement("exemple");

        let sortie = crochet(&mut evt, &npm, banc.chargeur());

        assert_eq!(sortie, Ok(Sortie::Construit));
        assert_eq!(banc.urls(), vec!["file:///cache/exemple/index.js".to_string()]);
        assert_eq!(evt.sdk, Some(json!({ "construit": true })));
    }

    #[test]
    fn l_ordre_des_exports_change_le_resultat_d_un_module_qui_expose_deux_fabriques() {
        // Un `BTreeMap` dedupliquerait par ordre lexicographique et choisirait
        // toujours `createA`. La source choisit le PREMIER dans l'ordre
        // d'enumeration, donc l'ordre est une donnee de comportement.
        let avant = vec!["default".to_string(), "createA".to_string(), "createB".to_string()];
        let apres = vec!["default".to_string(), "createB".to_string(), "createA".to_string()];

        assert_eq!(premier_export(&avant), Some("createA"));
        assert_eq!(premier_export(&apres), Some("createB"));
        assert_ne!(premier_export(&avant), premier_export(&apres));
    }

    #[test]
    fn seul_un_prefixe_exact_de_create_qualifie_un_export() {
        fn qualifie(noms: &[String]) -> Option<&str> {
            premier_export(noms)
        }
        assert_eq!(qualifie(&["create".to_string()]), Some("create"));
        assert_eq!(qualifie(&["createOpenAI".to_string()]), Some("createOpenAI"));
        assert_eq!(qualifie(&["default".to_string(), "createX".to_string()]), Some("createX"));
        // Ces cinq echouent : le prefixe est sensible a la casse, un prefixe
        // partiel ne suffit pas, et la chaine vide ne commence par rien.
        assert_eq!(qualifie(&["created".to_string()]), None);
        assert_eq!(qualifie(&["Create".to_string()]), None);
        assert_eq!(qualifie(&["creat".to_string()]), None);
        assert_eq!(qualifie(&["default".to_string(), "other".to_string()]), None);
        assert_eq!(qualifie(&["".to_string()]), None);
    }

    #[test]
    fn un_module_sans_export_qualifiant_leve_le_message_de_la_source() {
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        // La chaine vide est le cas qui distingue `!match` de "rien trouve" :
        // elle est falsy, mais `find` ne peut jamais la renvoyer, donc les deux
        // conditions coincident.
        let banc = Banc::nouveau(&["default", "", "other"]);
        let mut evt = evenement("exemple");

        let erreur = crochet(&mut evt, &npm, banc.chargeur())
            .expect_err("aucun export ne commence par create");

        assert_eq!(erreur, Erreur::AucuneFabrique { paquet: "exemple".to_string() });
        assert_eq!(erreur.to_string(), "Package exemple has no provider factory export");
        assert_eq!(
            banc.urls(),
            vec!["file:///cache/exemple/index.js".to_string()],
            "le module a bien ete importe avant l'echec"
        );
        assert_eq!(banc.appels(), 0, "la fabrique ne doit jamais etre appelee");
        assert!(evt.sdk.is_none());
    }

    // ------------------------------------------------ la fabrique et les options

    #[test]
    fn la_fabrique_recoit_exactement_les_options_de_l_evenement() {
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let banc = Banc::nouveau(&["createX"]);

        let mut options = Options::new();
        options.insert("apiKey".to_string(), json!("secret"));
        options.insert("baseURL".to_string(), json!("https://exemple.test"));
        let mut evt = SdkEvent::new("exemple", options);

        crochet(&mut evt, &npm, banc.chargeur()).unwrap();

        assert_eq!(banc.appels(), 1, "la fabrique est appelee exactement une fois");
        assert_eq!(banc.options_vues(), vec!["apiKey,baseURL".to_string()]);
        // Le plugin ne modifie pas les options : il n'y touche pas.
        assert_eq!(evt.options["apiKey"], json!("secret"));
        assert_eq!(evt.options["baseURL"], json!("https://exemple.test"));
    }

    #[test]
    fn une_option_vide_survit_pour_ce_plugin() {
        // Rien dans ce fichier ne teste une option en veracite : la chaine vide
        // est une valeur d'option comme une autre, elle est transmise telle
        // quelle a la fabrique. Un portage par un ternaire de veracite sur les
        // options aurait substitue un defaut et perdu le `baseURL` vide.
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let banc = Banc::nouveau(&["createX"]);

        let mut options = Options::new();
        options.insert("baseURL".to_string(), json!(""));
        let mut evt = SdkEvent::new("exemple", options);

        crochet(&mut evt, &npm, banc.chargeur()).unwrap();

        assert_eq!(banc.options_vues(), vec!["baseURL".to_string()]);
        assert_eq!(
            evt.options["baseURL"],
            json!(""),
            "la chaine vide n'est ni retiree ni remplacee"
        );
    }

    #[test]
    fn un_evenement_sans_option_est_bien_traite() {
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let banc = Banc::nouveau(&["createX"]);
        let mut evt = evenement_sans_options("exemple");

        let sortie = crochet(&mut evt, &npm, banc.chargeur());

        assert_eq!(sortie, Ok(Sortie::Construit));
        assert_eq!(
            banc.options_vues(),
            vec![String::new()],
            "un dictionnaire vide est transmis tel quel"
        );
        assert!(evt.sdk.is_some());
    }

    #[test]
    fn un_second_passage_cede_la_place_au_premier_resultat() {
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let premier_banc = Banc::nouveau(&["createX"]);
        let mut evt = evenement("exemple");

        let premier = crochet(&mut evt, &npm, premier_banc.chargeur());
        let instantane = evt.sdk.clone();

        // Le deuxieme passage arme un banc neuf, mais ne doit rien faire : le
        // SDK deja construit est truthy.
        let second_banc = Banc::nouveau(&["createY"]);
        let second = crochet(&mut evt, &npm, second_banc.chargeur());

        assert_eq!(premier, Ok(Sortie::Construit));
        assert_eq!(second, Ok(Sortie::Cede));
        assert_eq!(evt.sdk, instantane);
        assert!(second_banc.urls().is_empty(), "le deuxieme passage n'importe rien");
        assert_eq!(second_banc.appels(), 0);
        assert_eq!(npm.installes().len(), 1, "npm n'est appele qu'une fois");
    }

    // ---------------------------------------------- pathToFileURL, et son encodage

    #[test]
    fn un_chemin_absolu_devient_une_url_a_trois_barres() {
        assert_eq!(
            chemin_vers_url_fichier("/cache/@ai-sdk/anthropic/dist/index.mjs"),
            "file:///cache/@ai-sdk/anthropic/dist/index.mjs"
        );
        // Le compte de barres est le piege de cette fonction : un chemin POSIX
        // absolu commence deja par une barre, donc prefixer `file:///` a la
        // lettre donnerait `file:////cache`, quatre barres, que `import()`
        // refuse. Une seule barre de trop, invisible dans une comparaison
        // approximative.
        let url = chemin_vers_url_fichier("/cache/x.js");
        assert_eq!(url, "file:///cache/x.js");
        assert!(!url.starts_with("file:////"), "pas de barre superflue : {url}");
        assert_eq!(
            url.chars().filter(|c| *c == '/').count(),
            3,
            "deux barres du schema plus une du chemin : {url}"
        );
        // Idem pour la lettre de lecteur, qui n'a pas de barre initiale.
        let windows = chemin_vers_url_fichier("C:\\cache\\x.js");
        assert_eq!(windows, "file:///C:/cache/x.js");
        assert!(!windows.starts_with("file:////"), "pas de barre superflue : {windows}");
        // Le tiret, l'arobase, le point et la barre sont laisses tels quels : ce
        // sont des caracteres ordinaires d'un chemin de paquet npm.
        assert!(!caractere_echappe('-'));
        assert!(!caractere_echappe('@'));
        assert!(!caractere_echappe('.'));
        assert!(!caractere_echappe('/'));
        assert!(!caractere_echappe(':'), "la lettre de lecteur garde son deux-points");
    }

    #[test]
    fn un_chemin_windows_gagne_une_lettre_de_lecteur_et_des_barres_obliques() {
        // La lettre de lecteur reste dans le chemin, precedee du `/` que le
        // parseur ajoute apres l'autorite vide. C'est la forme que Node produit
        // et que `import()` accepte.
        assert_eq!(
            chemin_vers_url_fichier("C:\\cache\\packages\\exemple\\index.js"),
            "file:///C:/cache/packages/exemple/index.js"
        );
        assert_eq!(
            chemin_vers_url_fichier("c:/cache/x.js"),
            "file:///c:/cache/x.js",
            "la casse de la lettre est conservee"
        );
        // Une lettre seule n'est pas un lecteur : il n'y a pas de prefixe
        // special a ajouter.
        assert_eq!(chemin_vers_url_fichier("C:/"), "file:///C:/");
    }

    #[test]
    fn un_chemin_unc_donne_son_partage_comme_autorite() {
        assert_eq!(
            chemin_vers_url_fichier("\\\\partage\\dossier\\index.js"),
            "file://partage/dossier/index.js",
            "l'autorite remplace la premiere barre du chemin"
        );
        // Une autorite sans segment de chemin se termine par une barre.
        assert_eq!(chemin_vers_url_fichier("//serveur"), "file://serveur/");
    }

    #[test]
    fn les_caracteres_reserves_du_chemin_sont_percent_encodes() {
        // Comportement RETENU, pas verifie contre Node : voir l'entete.
        assert_eq!(
            chemin_vers_url_fichier("/cache/paquet@v1/index.mjs?a#b%c"),
            "file:///cache/paquet@v1/index.mjs%3Fa%23b%25c"
        );
        assert_eq!(
            chemin_vers_url_fichier("/cache/mon paquet/x.js"),
            "file:///cache/mon%20paquet/x.js"
        );
        // Non latin : UTF-8, deux octets, en majuscules.
        assert_eq!(
            chemin_vers_url_fichier("/cache/répertoire/x.js"),
            "file:///cache/r%C3%A9pertoire/x.js"
        );
        // Les separateurs Windows ne sont pas encodes : ils ont deja ete
        // transformes en barres obliques.
        assert_eq!(
            chemin_vers_url_fichier("C:\\a b\\c d.js"),
            "file:///C:/a%20b/c%20d.js"
        );
    }

    #[test]
    fn le_jeu_de_caracteres_echappes_est_fige() {
        for caractere in ['%', ' ', '"', '#', '<', '>', '?', '`', '[', ']', '^', '|', '{', '}'] {
            assert!(caractere_echappe(caractere), "{caractere} doit etre echappe");
        }
        for caractere in [
            'a', 'Z', '0', '9', '-', '_', '.', '~', '/', ':', '@', '+', '=', ',', ';', '!', '$', '&',
            '\'', '(', ')',
        ] {
            assert!(!caractere_echappe(caractere), "{caractere} doit rester litteral");
        }
        assert!(caractere_echappe('\n'), "controle C0");
        assert!(caractere_echappe('\u{0}'));
        assert!(caractere_echappe('\u{7F}'), "au-dessus de U+007E");
        assert!(caractere_echappe('é'));

        // Un chemin vide ne produit rien de particulier et ne panic pas.
        assert_eq!(chemin_vers_url_fichier(""), "file:///");

        // Les quartets d'octet sont stables.
        assert_eq!(nibble_en_hex(0), '0');
        assert_eq!(nibble_en_hex(9), '9');
        assert_eq!(nibble_en_hex(10), 'A');
        assert_eq!(nibble_en_hex(15), 'F');
    }

    // ---------------------------------------------- l'echange avec le TypeScript

    #[test]
    fn un_evenement_lu_depuis_le_json_du_typescript_est_traite() {
        // Le type est IMPORTE depuis provider_venice : ce test verifie donc
        // l'echange JSON une fois, a travers le type partage, ce qui est voulu.
        let brut = json!({
            "model": { "id": "exemple/modele-1" },
            "package": "exemple",
            "options": { "apiKey": "secret" }
        });
        let mut evt: SdkEvent = serde_json::from_value(brut).unwrap();
        assert!(evt.sdk.is_none());

        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let banc = Banc::nouveau(&["createX"]);
        crochet(&mut evt, &npm, banc.chargeur()).unwrap();

        assert_eq!(banc.urls(), vec!["file:///cache/exemple/index.js".to_string()]);
        // Les noms d'origine restent `model`, `package` et `options`, tous en
        // minuscules, donc sans renommage. Les cles d'options gardent la casse
        // du TypeScript.
        let objet = serde_json::to_value(&evt).unwrap();
        assert_eq!(objet["package"], json!("exemple"));
        assert_eq!(objet["options"]["apiKey"], json!("secret"));
        assert!(objet.get("api_key").is_none());
    }

    #[test]
    fn le_descripteur_du_plugin_enregistre_le_meme_crochet_que_la_fonction_libre() {
        let npm = NpmFaux::qui_resout("/cache/exemple/index.js");
        let banc = Banc::nouveau(&["createX"]);
        let mut evt = evenement("exemple");

        let sortie = Plugin::nouveau().enregistrer(&mut evt, &npm, banc.chargeur());

        assert_eq!(sortie, Ok(Sortie::Construit));
        assert_eq!(banc.urls(), vec!["file:///cache/exemple/index.js".to_string()]);
        assert_eq!(evt.sdk, Some(json!({ "construit": true })));
    }

    #[test]
    fn le_service_npm_est_utilisable_a_travers_un_soutrait() {
        // Le service est partage comme un service Effect : seul le contrat est
        // manipule, jamais une implementation concrete.
        let mut service: Box<dyn Npm> = Box::new(NpmFaux::qui_resout("/cache/x/index.js"));
        assert_eq!(service.add("exemple").unwrap(), Some("/cache/x/index.js".to_string()));
        assert_eq!(service.add("").unwrap(), Some("/cache/x/index.js".to_string()));
    }

    #[test]
    fn le_module_importe_se_lit_dans_un_echec_de_test() {
        let banc = Banc::nouveau(&["default", "createX"]);
        let module = banc.module();

        assert_eq!(
            module.noms(),
            ["default".to_string(), "createX".to_string()].as_slice()
        );
        assert_eq!(premier_export(module.noms()), Some("createX"));
        let rendu = format!("{module:?}");
        assert!(rendu.contains("createX"), "le debug doit montrer les exports : {rendu}");
    }
}
