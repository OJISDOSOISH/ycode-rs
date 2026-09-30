//! Portage Rust de `opencode/packages/core/src/npm-config.ts` (40 lignes).
//!
//! # Ce qui ne se porte pas, et pourquoi
//!
//! La source fait deux choses : elle instancie `@npmcli/config`, qui LIT des
//! fichiers `.npmrc` (projet, utilisateur, global) et interroge l
//! environnement, puis elle normalise la valeur de `registry`.
//!
//! Le lecteur de `.npmrc` NE SE PORTE PAS, et c est un choix, pas un oubli.
//! Trois raisons, dans l ordre d importance : il exigerait un acces au
//! systeme de fichiers, interdit ici par le cahier des charges ; il
//! exigerait un acces a l environnement du processus ; et `@npmcli/config`
//! n est PAS installe sur cette machine (le repertoire `node_modules`
//! pour le paquet `@npmcli` est absent), donc ses definitions, son `flatten`
//! et ses `nerfDarts` ne sont pas consultables, et les regles exactes du
//! parcours `.npmrc` vers config plate ne sont pas verifiables ici, seulement
//! deduites.
//!
//! Ce qui se porte, c est la logique : [`charger`] reproduit l avalage
//! d erreur de `Effect.orElseSucceed(() => ({}))`, et [`registre`] reproduit la
//! normalisation. La lecture reelle est injectee par le premier argument, ce
//! qui rend la fonction pure et testable sans disque et sans processus.
//!
//! # Piege 1 : les noms de champs sont en camelCase, pas en snake_case
//!
//! La config exposee est `config.flat`, donc les noms vus par le TypeScript
//! sont ceux de npm. `packages/core/test/npm-config.test.ts` le prouve sur
//! trois cas concrets, a partir d un `.npmrc` ecrit en kebab-case.
//! `ignore-scripts=true` devient `config.ignoreScripts`, en camelCase.
//! `omit[]=dev` devient `config.omit`, un tableau.
//! `@acme:registry=...` reste `@acme:registry`, cle de portee.
//!
//! Un portage qui declare `ignore_scripts` sans `#[serde(rename)]` ne casse
//! rien a la compilation : Rust ne voit pas de JSON ici. Ca ne casse que a
//! l echange avec le TypeScript, silencieusement. D ou les deux tests dedies
//! en fin de fichier : l un ecrit la forme exacte, l autre REFUSE la forme
//! snake_case a la lecture.
//!
//! Note : contrairement a ce que la consigne du lot anticipait, ce fichier
//! n a AUCUN nom de champ en MAJUSCULES. Le seul nom lu par la source est
//! `registry`, deja minuscule des deux cotes, donc le seul `rename` pose ici
//! est defensif et explicite. Le vrai piege est l inverse de celui annonce :
//! la casse ne se voit pas, elle se paie a l execution.
//!
//! # Piege 2 : le test de TYPE n est pas le test de VERACITE, ni le `??`
//!
//! La source ecrit `typeof config.registry === "string" ? config.registry :
//! defaut`. Ce n est ni un ternaire de veracite, qui rejetterait `""`, ni un
//! `??`, qui ne testerait que la nullite. C est un test de TYPE. Concretement
//! sur une config npm, qui compte beaucoup de valeurs optionnelles :
//! `registry: ""` passe la garde de type et rend `""`, alors qu un test de
//! veracite rendrait le defaut ; c est la seule divergence entre les deux
//! portes, et elle est intentionnelle.
//! `registry: null` rend le defaut, car `typeof null` vaut `object`.
//! `registry: 42` rend le defaut aussi : `42` n est pas une chaine, et la
//! garde refuse tout ce qui n en est pas une.
//!
//! Les deux portes de sortie sont donc deux fonctions distinctes et jamais
//! une seule : [`lire_champ_chaine`] (garde de type) et [`defaut_si_absente`]
//! (coalescent `??`). Les fusionner ferait diverger le cas `""`.
//!
//! # Piege 3 : le slicing par index d octet
//!
//! `registry.slice(0, -1)` se traduit naivement par `&s[..s.len() - 1]`, qui
//! PANIQUE des que la coupe tombe au milieu d un caractere multi-octets
//! (accent, CJK, emoji). Ici la coupe n est protegee que par le `endsWith("/")`
//! qui la precede, et `/` ne peut pas apparaitre dans un caractere UTF-8
//! multi-octets, donc la version naive ne paniquerait PAS sur cette ligne
//! precise. On utilise quand meme [`sans_barre_finale`], fondee sur
//! `strip_suffix`, qui rend la panique impossible par construction plutot que
//! par un raisonnement. Le test `une_coupe_par_octet_paniquerait_sur_un_accent`
//! demontre le piege sur une chaine reelle, et
//! `une_url_non_ascii_ne_panique_pas` verifie que le portage est sur.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Le registre npm public, valeur de repli quand la config n en dit rien.
///
/// La source ecrit la chaine en dur dans le ternaire. On la constante pour que
/// les tests et les appelants partagent exactement la meme valeur.
pub const REGISTRE_PAR_DEFAUT: &str = "https://registry.npmjs.org";

/// La configuration plate de npm, equivalente de `Record<string, unknown>`.
///
/// Un sac de paires, volontairement non type, comme dans la source. Le type
/// `Value` est le seul qui accepte un JSON heterogene sans conversion.
pub type ConfigPlat = BTreeMap<String, Value>;

/// Le seul champ que la source lit, sous forme typee et stricte.
///
/// Cette struct est la VUE ETROITE du sac : elle sert a figer le contrat de
/// nommage JSON, pas a representer les dizaines de cles que npm peut produire.
///
/// `deny_unknown_fields` est ce qui rend possible le test de refus de la forme
/// snake_case : sans lui, serde ignore silencieusement `{ "registry_url": ... }`
/// et l on ne verrait jamais le defaut de nommage. Il ne s applique qu a cette
/// struct et jamais a [`ConfigPlat`], qui doit pouvoir absorber les cinquante
/// cles de npm sans en echouer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registre {
    /// Nom JSON : minuscule exact, comme `config.registry` en TypeScript.
    ///
    /// Le `rename` est redondant avec le nom du champ, il est donc pose
    /// volontairement : c est la piece a relire si quelqu un renomme le champ
    /// en `registry_url` ou en `Registry`, qui compileraient tous les deux.
    #[serde(rename = "registry", default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
}

impl Registre {
    /// Construit la vue stricte depuis le sac plat, sans jamais echouer.
    ///
    /// Un champ absent, nul ou d un autre type donne `None`, ce qui n est pas
    /// une erreur : la source ignore silencieusement tout ce qui n est pas une
    /// chaine.
    pub fn depuis(config: &ConfigPlat) -> Registre {
        Registre {
            registry: lire_champ_chaine(config, "registry").map(str::to_string),
        }
    }

    /// Le registre retenu, ou le defaut si la vue ne contient rien.
    pub fn effectif(&self) -> &str {
        defaut_si_absente(self.registry.as_deref(), REGISTRE_PAR_DEFAUT)
    }
}

/// Portage de `load`.
///
/// La source fait `Effect.tryPromise({...}).pipe(Effect.orElseSucceed(() =>
/// ({})))`. Le sens est simple et IMPORTANT : la moindre erreur de lecture de
/// la config npm ne remonte PAS, elle est remplacee par une config vide. Il n y
/// a donc aucun cas ou `load` echoue.
///
/// `lecture` est l equivalent de la lecture de `.npmrc` par `@npmcli/config`.
/// Elle est injectee, donc cette fonction ne touche ni le disque ni
/// l environnement et se teste avec une simple closure.
///
/// `dir` est transmis tel quel a `lecture`, comme le `cwd` de la source, mais
/// n est jamais lu autrement.
pub fn charger<F, E>(dir: &str, lecture: F) -> ConfigPlat
where
    F: FnOnce(&str) -> Result<ConfigPlat, E>,
{
    // `orElseSucceed(() => ({}))` : toute erreur devient une config vide.
    // `unwrap_or_default` fait exactement cela, sans panic ni type de sortie
    // supplementaire. Le parametre `E` disparait, comme dans la source.
    lecture(dir).unwrap_or_default()
}

/// Portage du `flatten` de npm, en reconstitution.
///
/// Les noms de cles du `.npmrc` sont en kebab-case, ceux de la config plate
/// sont en camelCase. On ne reproduit QUE la conversion de nom, parce que
/// c est elle qui casse silencieusement a l echange avec le TypeScript.
///
/// Limite assumee et signalee : le parsing `.npmrc` lui-meme (syntaxe ini,
/// `cle[]=valeur` pour les tableaux, heritage projet/utilisateur/global,
/// `nerfDarts` sur les jetons) ne se porte pas, et les regles exactes du
/// `flatten` npm ne sont pas verifiables ici, la bibliotheque n etant pas
/// installee. Les trois cas ci-dessous sont ceux qu atteste
/// `packages/core/test/npm-config.test.ts`.
///
/// Regles appliquees, dans l ordre :
/// une cle contenant `-` devient camelCase, donc `ignore-scripts` donne
/// `ignoreScripts`.
/// Toute autre cle est laissee intacte, donc les cles de portee comme
/// `@acme:registry` et les cles contenant `_` ou `.` ne bougent pas.
/// Une cle deja presente gagne sur la forme convertie, pour qu une conversion
/// ne puisse pas ecraser une cle exacte.
pub fn aplatir(brut: &BTreeMap<String, Value>) -> ConfigPlat {
    let mut resultat = ConfigPlat::new();
    for (cle, valeur) in brut {
        if !cle.contains('-') {
            resultat.insert(cle.clone(), valeur.clone());
        }
    }
    for (cle, valeur) in brut {
        if cle.contains('-') {
            let converti = cle_en_camel(cle);
            resultat.entry(converti).or_insert_with(|| valeur.clone());
        }
    }
    resultat
}

/// `ignore-scripts` devient `ignoreScripts`, `save-prefix` devient
/// `savePrefix`.
///
/// Chaque segment apres le premier est capitalise sur sa premiere lettre, sans
/// toucher au reste : `omit-optional-dev` devient `omitOptionalDev`.
fn cle_en_camel(cle: &str) -> String {
    let mut sortie = String::with_capacity(cle.len());
    for (index, segment) in cle.split('-').enumerate() {
        if index == 0 {
            sortie.push_str(segment);
        } else {
            let mut caracteres = segment.chars();
            if let Some(premier) = caracteres.next() {
                sortie.extend(premier.to_uppercase());
                sortie.push_str(caracteres.as_str());
            }
        }
    }
    sortie
}

/// Le test de TYPE de la source : `typeof config[cle] === "string"`.
///
/// Fonction distincte de [`defaut_si_absente`] et c est le point du fichier.
/// Elle rend `Some` UNIQUEMENT pour une vraie chaine JSON. Sont donc refuses,
/// comme en JavaScript : la cle absente, qui vaut `undefined` ; `null`, car
/// `typeof null` vaut `object` ; `true`, `42`, `[]` et `{}`, qui sont des
/// valeurs mais pas des chaines. Et surtout `""`, qui EST une chaine et donc
/// ACCEPTEE, la ou un test de veracite l aurait refusee.
pub fn lire_champ_chaine<'a>(config: &'a ConfigPlat, cle: &str) -> Option<&'a str> {
    match config.get(cle) {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// Le coalescent `??` de la source, dans une fonction a part.
///
/// Il ne teste QUE la nullite : `Some("")` ressort `""`. Confondu avec
/// [`lire_champ_chaine`], il donnerait le defaut pour une chaine vide, ce qui
/// changerait la sortie de `registry` sur une entree reellement possible.
pub fn defaut_si_absente<'a>(valeur: Option<&'a str>, defaut: &'a str) -> &'a str {
    valeur.unwrap_or(defaut)
}

/// Portage de `registry.endsWith("/") ? registry.slice(0, -1) : registry`.
///
/// Retire UNE barre oblique finale, pas toutes : `"https://x//"` devient
/// `"https://x/"`, exactement comme `slice(0, -1)` en JavaScript.
///
/// On utilise `strip_suffix` et NON `&s[..s.len() - 1]`. La raison est
/// documentee dans l en-tete : la coupe par index d octet peut tomber au
/// milieu d un caractere multi-octets et paniquer. Ici `/` est ASCII, donc la
/// coupe naive serait tombee sur une frontiere, mais cela tient a une
/// propriete du cas particulier et non a une garantie du code. `strip_suffix`
/// rend la panique impossible.
pub fn sans_barre_finale(registre: &str) -> &str {
    registre.strip_suffix('/').unwrap_or(registre)
}

/// Portage de `registry(dir)`, une fois la config chargee.
///
/// Reproduit les deux etapes de la source :
/// 1. garde de TYPE sur `config.registry`, sinon le defaut ;
/// 2. retrait d une eventuelle barre oblique finale.
pub fn registre(config: &ConfigPlat) -> String {
    let retenu = defaut_si_absente(lire_champ_chaine(config, "registry"), REGISTRE_PAR_DEFAUT);
    sans_barre_finale(retenu).to_string()
}

/// Portage de l export `registry` de la source, de bout en bout.
///
/// `charge` joue le role de `load`, avec la lecture injectee. La composition
/// est la meme que dans le TS : charger, puis normaliser.
pub fn registre_depuis<F, E>(dir: &str, charge: F) -> String
where
    F: FnOnce(&str) -> Result<ConfigPlat, E>,
{
    registre(&charger(dir, charge))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un sac plat depuis un JSON, sans toucher au disque.
    fn sac(json: &str) -> ConfigPlat {
        serde_json::from_str(json).expect("json de test invalide")
    }

    /// Une lecture qui echoue, comme le ferait un `.npmrc` illisible.
    ///
    /// C est un item de fonction, pas une closure : sa signature est concrete,
    /// donc aucune inference de type n est necessaire a l appel de [`charger`].
    fn lecture_qui_echoue(_dir: &str) -> Result<ConfigPlat, &'static str> {
        Err(".npmrc illisible")
    }

    // -----------------------------------------------------------------------
    // Piege 2 : test de type contre test de veracite, contre coalescent
    // -----------------------------------------------------------------------

    #[test]
    fn une_chaine_vide_est_conservee_par_le_garde_de_type() {
        // Le coeur du piege. `typeof config.registry === "string"` accepte `""`
        // parce que `""` EST une chaine. Un portage par test de veracite
        // rendrait ici le registre public, ce qui est faux : l utilisateur a
        // explicitement ecrit une valeur.
        assert_eq!(registre(&sac(r#"{"registry":""}"#)), "");
    }

    #[test]
    fn le_garde_de_type_et_le_coalescent_divergent_sur_la_chaine_vide() {
        // Les deux portes de sortie ne sont pas la meme fonction, et le
        // tableau ci-dessous est la raison. Si quelqu un unifie les deux,
        // cette assertion casse.
        assert_eq!(lire_champ_chaine(&sac(r#"{"registry":""}"#), "registry"), Some(""));
        assert_eq!(defaut_si_absente(Some(""), REGISTRE_PAR_DEFAUT), "");
        // Et sur une vraie absence, les deux convergent, ce qui masque l erreur
        // si on ne regarde que ce cas-la.
        assert_eq!(lire_champ_chaine(&sac("{}"), "registry"), None);
        assert_eq!(defaut_si_absente(None, REGISTRE_PAR_DEFAUT), REGISTRE_PAR_DEFAUT);
    }

    #[test]
    fn seule_une_valeur_reellement_chaine_passe_la_garde() {
        // `typeof null` vaut "object" en JavaScript, donc `null` prend le
        // defaut. Les autres types ne sont pas des chaines non plus.
        assert_eq!(registre(&sac("{}")), REGISTRE_PAR_DEFAUT);
        assert_eq!(registre(&sac(r#"{"registry":null}"#)), REGISTRE_PAR_DEFAUT);
        assert_eq!(registre(&sac(r#"{"registry":true}"#)), REGISTRE_PAR_DEFAUT);
        assert_eq!(registre(&sac(r#"{"registry":42}"#)), REGISTRE_PAR_DEFAUT);
        assert_eq!(registre(&sac(r#"{"registry":["https://x.test"]}"#)), REGISTRE_PAR_DEFAUT);
        assert_eq!(registre(&sac(r#"{"registry":{"url":"https://x.test"}}"#)), REGISTRE_PAR_DEFAUT);
    }

    #[test]
    fn un_registre_non_string_renvoie_la_chaine_par_defaut() {
        // Memes entrees que ci-dessus, passees par la vue typee : le champ
        // reste absent, donc le defaut s applique par l autre porte.
        let r = Registre::depuis(&sac(r#"{"registry":42}"#));
        assert_eq!(r.registry, None);
        assert_eq!(r.effectif(), REGISTRE_PAR_DEFAUT);
    }

    // -----------------------------------------------------------------------
    // Normalisation du registre
    // -----------------------------------------------------------------------

    #[test]
    fn une_une_barre_finale_seule_est_retiree() {
        assert_eq!(registre(&sac(r#"{"registry":"https://registry.example.test/"}"#)), "https://registry.example.test");
    }

    #[test]
    fn une_barre_finale_unique_est_retiree() {
        // `slice(0, -1)` ne retire qu UN caractere. Le cas `//` est la preuve
        // que le portage ne fait pas un `trim_end_matches('/')`.
        assert_eq!(registre(&sac(r#"{"registry":"https://registry.example.test//"}"#)), "https://registry.example.test/");
    }

    #[test]
    fn un_registre_sans_barre_finale_est_inchange() {
        assert_eq!(registre(&sac(r#"{"registry":"https://registry.example.test"}"#)), "https://registry.example.test");
    }

    #[test]
    fn une_barre_interne_nest_jamais_touchee() {
        // Le retrait est suffixal, pas un nettoyage general.
        assert_eq!(sans_barre_finale("https://a.test/b/c"), "https://a.test/b/c");
        assert_eq!(sans_barre_finale("/"), "");
        assert_eq!(sans_barre_finale(""), "");
    }

    // -----------------------------------------------------------------------
    // Avalage d erreur de `load`
    // -----------------------------------------------------------------------

    #[test]
    fn une_erreur_de_lecture_donne_une_config_vide_sans_panique() {
        // `Effect.orElseSucceed(() => ({}))` : aucune erreur ne sort de `load`.
        // Le type de retour le prouve deja (pas de `Result`), et ce test
        // verifie le comportement.
        let config = charger("dossier", lecture_qui_echoue);
        assert!(config.is_empty());
        assert_eq!(registre(&config), REGISTRE_PAR_DEFAUT);
    }

    #[test]
    fn le_repertoire_est_transmis_a_la_lecture() {
        // Le `cwd` de la source, on le respecte : la lecture injectee doit
        // recevoir exactement la chaine fournie.
        let config = charger("/un/chemin/precis", |dir: &str| -> Result<ConfigPlat, ()> {
            assert_eq!(dir, "/un/chemin/precis");
            Ok(sac(r#"{"registry":"https://ok.test/"}"#))
        });
        assert_eq!(registre(&config), "https://ok.test");
    }

    #[test]
    fn la_lecture_n_est_appellee_qu_une_seule_fois() {
        // `FnOnce` dans la signature : le compilateur interdit deja le double
        // appel, ce test verrouille l intention pour le relecteur.
        let mut appels = 0;
        let config = charger("dossier", |_dir: &str| -> Result<ConfigPlat, ()> {
            appels += 1;
            Ok(ConfigPlat::new())
        });
        assert_eq!(appels, 1);
        assert_eq!(registre(&config), REGISTRE_PAR_DEFAUT);
    }

    #[test]
    fn registre_depuis_compose_les_deux_etapes() {
        assert_eq!(
            registre_depuis("dossier", |_dir: &str| -> Result<ConfigPlat, ()> {
                Ok(sac(r#"{"registry":"https://compose.test/"}"#))
            }),
            "https://compose.test"
        );
        // Chemin d erreur : config vide, donc defaut, comme dans la source.
        assert_eq!(
            registre_depuis("dossier", |_dir: &str| -> Result<ConfigPlat, ()> { Err(()) }),
            REGISTRE_PAR_DEFAUT
        );
    }

    // -----------------------------------------------------------------------
    // Piege 1 : nommage JSON, ecriture
    // -----------------------------------------------------------------------

    #[test]
    fn une_serie_ecrit_la_cle_exacte_en_minuscules() {
        // Le JSON doit contenir la cle que le TypeScript lit, c est-a-dire
        // `config.registry` en minuscule. Ni `Registry`, ni `REGISTRY`, ni
        // `registry_url`.
        let r = Registre {
            registry: Some("https://registry.example.test".to_string()),
        };
        let json = serde_json::to_string(&r).expect("serialisation");
        assert_eq!(json, r#"{"registry":"https://registry.example.test"}"#);
    }

    #[test]
    fn un_champ_absent_nest_pas_serie() {
        // `skip_serializing_if` : un `Option` a `None` ne produit pas de cle,
        // donc on n invente pas un `registry: null` que le TS ne lit pas.
        let json = serde_json::to_string(&Registre::default()).expect("serialisation");
        assert_eq!(json, "{}");
    }

    // -----------------------------------------------------------------------
    // Piege 1 : nommage JSON, refus de la forme snake_case
    // -----------------------------------------------------------------------

    #[test]
    fn la_forme_snake_case_est_refusee_a_la_lecture() {
        // C'est le test le plus important du fichier pour l echange avec le
        // TypeScript : une erreur de nommage est INVISIBLE a la compilation.
        // `deny_unknown_fields` transforme le silence en echec franc.
        assert!(serde_json::from_str::<Registre>(r#"{"registry_url":"https://x.test"}"#).is_err());
        assert!(serde_json::from_str::<Registre>(r#"{"ignore_scripts":true}"#).is_err());
    }

    #[test]
    fn une_mauvaise_casse_est_refusee_a_la_lecture() {
        // La consigne du lot annoncait des MAJUSCULES. Ce fichier n en a pas,
        // mais la faute reste possible et silencieuse, donc elle est testee.
        assert!(serde_json::from_str::<Registre>(r#"{"Registry":"https://x.test"}"#).is_err());
        assert!(serde_json::from_str::<Registre>(r#"{"REGISTRY":"https://x.test"}"#).is_err());
    }

    #[test]
    fn la_forme_correcte_est_acceptee() {
        let r: Registre = serde_json::from_str(r#"{"registry":"https://ok.test"}"#).expect("forme attendue");
        assert_eq!(r.registry.as_deref(), Some("https://ok.test"));
        // Et l objet vide est accepte aussi, puisque le champ est optionnel.
        let vide: Registre = serde_json::from_str("{}").expect("objet vide");
        assert_eq!(vide.registry, None);
        assert_eq!(vide.effectif(), REGISTRE_PAR_DEFAUT);
    }

    // -----------------------------------------------------------------------
    // Piege 3 : slicing par index d octet
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "is not a char boundary")]
    fn une_coupe_par_octet_paniquerait_sur_un_accent() {
        // Demonstration du piege annonce pour ce lot. `e` accentue vaut deux
        // octets, donc `len() - 1` tombe ENTRE les deux et la coupe n est pas
        // sur une frontiere de caractere : Rust panique.
        //
        // Ce test ne teste pas le portage, il fixe la raison pour laquelle le
        // portage n utilise pas `&s[..s.len() - 1]`. Il attend explicitement
        // la panique, et attend aussi que l `assert` d avant ne déclenche
        // RIEN : si un jour la coupe devenait legale, ce test echouerait
        // sur le message, ce qui est le comportement voulu.
        let s = "cafe\u{e9}";
        let coupe = s.len() - 1;
        assert!(!s.is_char_boundary(coupe), "la coupe doit etre illegale");
        let _panique = &s[..coupe];
    }

    #[test]
    fn une_url_non_ascii_ne_panique_pas() {
        // Le portage reel, sur des entrees qui casseraient une coupe naive.
        // Accent, CJK et emoji, tous avec et sans barre finale.
        assert_eq!(
            registre(&sac("{\"registry\":\"https://caf\u{e9}.test/\"}")),
            "https://caf\u{e9}.test"
        );
        assert_eq!(
            registre(&sac("{\"registry\":\"https://registry.example.test/\u{1f600}\"}")),
            "https://registry.example.test/\u{1f600}"
        );
        assert_eq!(registre(&sac("{\"registry\":\"https://\u{4f60}\u{597d}.test/\"}")), "https://\u{4f60}\u{597d}.test");
        // La barre finale retiree ne doit laisser aucun octet casse : le
        // resultat reste du texte UTF-8 valide et se compare chaine a chaine.
        let r = registre(&sac("{\"registry\":\"https://caf\u{e9}.test/\u{1f600}/\"}"));
        assert_eq!(r, "https://caf\u{e9}.test/\u{1f600}");
    }

    // -----------------------------------------------------------------------
    // Nommage des cles plates : kebab a la lecture, camel a l ecriture
    // -----------------------------------------------------------------------

    #[test]
    fn un_nom_de_cle_tiret_vient_camelise() {
        // Cas atteste par `packages/core/test/npm-config.test.ts` :
        // `ignore-scripts=true` dans le .npmrc, `config.ignoreScripts` ensuite.
        let plat = aplatir(&sac(r#"{"ignore-scripts":true,"save-prefix":"","user-config":"/x"}"#));
        assert_eq!(plat.get("ignoreScripts"), Some(&Value::Bool(true)));
        assert_eq!(plat.get("savePrefix"), Some(&Value::String(String::new())));
        assert_eq!(plat.get("userConfig"), Some(&Value::String("/x".to_string())));
    }

    #[test]
    fn une_cle_de_portee_est_conservee_telle_quelle() {
        // Cas atteste par le meme test : `@acme:registry` n est pas transforme.
        let plat = aplatir(&sac(r#"{"@acme:registry":"https://npm.acme.test/"}"#));
        assert_eq!(plat.get("@acme:registry"), Some(&Value::String("https://npm.acme.test/".to_string())));
    }

    #[test]
    fn une_cle_exacte_gagne_sur_la_forme_convertie() {
        // Si le sac contient les deux ecritures, la conversion ne doit pas
        // ecraser la cle deja conforme.
        let plat = aplatir(&sac(r#"{"ignore-scripts":false,"ignoreScripts":true}"#));
        assert_eq!(plat.get("ignoreScripts"), Some(&Value::Bool(true)));
    }

    #[test]
    fn le_registre_champ_est_deja_plat_et_passe_a_travers() {
        // `registry` ne contient ni `-` ni `_` : aucune conversion ne doit
        // s y appliquer, sinon la lecture du registre deviendrait fausse.
        let plat = aplatir(&sac(r#"{"registry":"https://plat.test/"}"#));
        assert_eq!(registre(&plat), "https://plat.test");
    }

    #[test]
    fn aplatir_conserve_les_valeurs_non_chaines() {
        // `omit[]=dev` donne un TABLEAU cote npm. Le type de valeur ne change
        // pas quand on change le nom de cle.
        let plat = aplatir(&sac(r#"{"omit":["dev","optional"]}"#));
        let attendu = serde_json::json!(["dev", "optional"]);
        assert_eq!(plat.get("omit"), Some(&attendu));
    }
}