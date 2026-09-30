//! Portage de `packages/core/src/markdown.d.ts`.
//!
//! La source tient en quatre lignes et ne contient aucune logique :
//!
//! ```ts
//! declare module "*.md" {
//!   const content: string
//!   export default content
//! }
//! ```
//!
//! C'est une **declaration de module ambiant** : elle ne decrit pas un fichier a
//! executer, elle **ajoute** au compilateur TypeScript la regle "si un
//! specificateur se termine par `.md`, alors l'import de ce specificateur a un
//! export par defaut de type `string`". Le contenu de la chaine n'est pas
//! embarque dans la declaration : c'est le bundler qui lit le fichier et
//! substitue son texte. La declaration ne dit donc rien du contenu, seulement
//! de sa forme.
//!
//! Ce que le fichier porte reellement, c'est un contrat de **forme**, et c'est
//! la seule chose traduisible : un type et un motif. D'ou les deux entites
//! ci-dessous, [`SPECIFICATEUR`] et [`Markdown`]. Rien d'autre n'est ajoute : ni
//! analyseur de Markdown, ni rendu, ni lecture de fichier, parce que rien de
//! tout cela n'existe dans la source. Ce fichier est lui-meme 100% ASCII : les
//! chaines multi-octets des tests sont ecrites en echappements Unicode, pour
//! qu'un fichier marque UTF-8 ou Latin-1 ne change rien a leur valeur.
//!
//! # Le contrat de nommage est la source
//!
//! La declaration vit dans un `.d.ts` : ce n'est pas du code, c'est une
//! declaration. Le type exporte s'appelle `string`, ce n'est donc pas un objet
//! mais une chaine nue, et **il n'y a aucun nom de champ du tout**. Le piege
//! habituel du lot (un `CONTENT` ou un `content` en MAJUSCULES qui passe la
//! compilation et casse l'echange avec le TypeScript) ne peut donc pas se
//! poser tel quel ici. Le piege inverse, lui, est bien reel : modeliser
//! l'export par defaut comme un struct `{ content: String }` ne declenche
//! **aucune** erreur de compilation, produit `{"content": ...}` au lieu de
//! `"..."`, et casse l'echange de la meme facon, en silence. Le type est donc
//! un **newtype transparent** : serde ecrit et lit une chaine JSON nue.
//! Deux tests verrouillent cela, un test d'ecriture et un test qui refuse la
//! forme objet.
//!
//! # `?` contre `??`
//!
//! La source ne contient ni ternaire ni coalescent : il n'y a donc aucune
//! fonction a porter de ce cote, et aucune n'est inventee ici. Le contrat
//! comporte malgre tout une information utile a ce sujet, ecrite en
//! commentaire : l'export par defaut d'un module ambiant est **toujours
//! present**. `content` n'est ni `null` ni `undefined`.
//!
//! Consequence, si un appelant ecrit `contenu ?? valeur_de_repli` en
//! TypeScript, la branche de droite est du code mort. En revanche `contenu ||
//! valeur_de_repli` reste vivant, parce que `||` teste la **veracite** et que la
//! chaine vide est falsy. Un fichier `.md` vide est un cas reel, pas une
//! hypothese : il produit une chaine vide qui doit survivre a l'aller-retour.
//! Un test dedie verrouille cette survival, et `est_vide` est expose pour que
//! l'appelant ait un moyen explicite de la tester au lieu d'ecrire `!` sur une
//! chaine.
//!
//! # Limite assumee
//!
//! Cette declaration est consommee par un seul import dans tout le depot,
//! `packages/core/src/plugin/skill.ts:9`, qui lie son export par defaut a la
//! constante `CustomizeOpencodeContent` et le place dans le champ `content`
//! d'un `Info`. Ce champ est un `String` nu dans
//! [`crate::swarm::plugin_skill`], pas un [`Markdown`] : les deux ne sont pas
//! le meme contrat et ne doivent pas etre unifies. Le type contenu ici decrit
//! l'export du module ambiant, pas le champ du skill.

use serde::{Deserialize, Serialize};

/// Motif du specificateur que la declaration de module ambiant reconnait.
///
/// C'est le seul litteral de la source. Le `*` d'un motif de module ambiant
/// TypeScript n'est pas le `*` de minimatch : il ne s'arrete ni sur `/` ni sur
/// une chaine vide. Le motif se reduit donc a "se termine par `.md`", ce que
/// [`Markdown::correspond_a`] applique.
pub const SPECIFICATEUR: &str = "*.md";

/// Export par defaut d'un module `*.md`.
///
/// Equivalent de la declaration `const content: string` : une chaine nue, sans
/// enveloppe, sans nom de champ, sans valeur par defaut. Le contenu vient du
/// fichier lu au build, jamais de la declaration.
///
/// Le type est un newtype transparent : sur le fil, il se serialise et se
/// deserialise en **chaine JSON nue**, exactement comme le ferait le `string`
/// TypeScript, et refuse tout objet, nombre, booleen ou `null`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Markdown(String);

impl Markdown {
    /// Construit un contenu a partir du texte lu dans un fichier `.md`.
    ///
    /// Le nom `nouveau` reflete le fait qu'il n'existe pas de constructeur
    /// dans la source : la valeur est produite par le bundler. La chaine vide
    /// est acceptee et reste distinguishable d'un contenu absent.
    pub fn nouveau(contenu: impl Into<String>) -> Self {
        Markdown(contenu.into())
    }

    /// Texte du contenu, en lecture seulement.
    ///
    /// Renvoie un `&str`, jamais un index. Un `.md` est du texte UTF-8 : une
    /// troncature par index d'octet (`&s[..n]`) coupe au milieu d'un caractere
    /// et **panique a l'execution**. Aucun slicing de ce genre n'est expose ici,
    /// et le test `tronquer_sur_un_index_octet_...` le demontre.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Recupere la chaine interne sans copie supplementaire.
    pub fn into_string(self) -> String {
        self.0
    }

    /// Le fichier est-il vide ?
    ///
    /// Correspond a la veracite JavaScript d'une chaine vide. Attention : la
    /// chaine vide est **presente**, donc un `??` ne se declenche pas ici, seul
    /// un `||` le ferait. Voir le commentaire de tete de module.
    pub fn est_vide(&self) -> bool {
        self.0.is_empty()
    }

    /// Le specificateur tombe-t-il sous le motif `*.md` ?
    ///
    /// Question posee par le seul litteral de la source. Ne resout aucun
    /// chemin, ne touche pas au disque : `correspond_a("guide.md")` repond
    /// `true` sans rien lire, exactement comme le fait la resolution de
    /// modules. La casse compte, `.MD` ne correspond pas.
    pub fn correspond_a(specificateur: &str) -> bool {
        specificateur.ends_with(".md")
    }
}

#[cfg(test)]
mod tests {
    use super::{Markdown, SPECIFICATEUR};

    #[test]
    fn le_motif_declare_est_bien_etoile_point_m_d() {
        assert_eq!(SPECIFICATEUR, "*.md");
    }

    #[test]
    fn le_motif_couvre_les_specificateurs_md_et_eux_seuls() {
        // Le `*` d'un module ambiant ne s'arrete ni sur `/` ni sur la vide.
        assert!(Markdown::correspond_a("./skill/customize-opencode.md"));
        assert!(Markdown::correspond_a("/builtin/customize-opencode.md"));
        assert!(Markdown::correspond_a("guide.md"));
        assert!(Markdown::correspond_a(".md"));
        assert!(!Markdown::correspond_a("guide.mdx"));
        assert!(!Markdown::correspond_a("guide.MD"));
        assert!(!Markdown::correspond_a("guide.md.ts"));
        assert!(!Markdown::correspond_a("guide"));
        assert!(!Markdown::correspond_a(""));
    }

    #[test]
    fn le_contenu_serialise_en_chaine_json_nue() {
        let m = Markdown::nouveau("# Titre\n\nCorps.");
        let json = serde_json::to_string(&m).expect("serialisation");
        assert_eq!(json, "\"# Titre\\n\\nCorps.\"");
    }

    #[test]
    fn le_contenu_ne_serialise_pas_en_objet_avec_un_champ_content() {
        // Le piege du lot : un nom de champ en MAJUSCULES, ou en minuscules,
        // passerait la compilation et casserait l'echange sans lever d'erreur.
        let json = serde_json::to_string(&Markdown::nouveau("x")).expect("serialisation");
        assert!(!json.contains("content"), "objet inattendu : {json}");
        assert!(!json.contains("CONTENT"), "objet inattendu : {json}");
        assert!(!json.contains("Content"), "objet inattendu : {json}");
        assert!(!json.contains('{'), "le contrat est une chaine nue : {json}");
    }

    #[test]
    fn la_lecture_refuse_la_forme_objet_snake_case() {
        for json in [r#"{"content":"x"}"#, r#"{"default":"x"}"#, r#"{"value":"x"}"#] {
            assert!(
                serde_json::from_str::<Markdown>(json).is_err(),
                "forme objet acceptee alors que le contrat est une chaine : {json}"
            );
        }
    }

    #[test]
    fn la_lecture_refuse_la_forme_objet_en_majuscules() {
        for json in [r#"{"CONTENT":"x"}"#, r#"{"Content":"x"}"#, r#"{"Default":"x"}"#] {
            assert!(
                serde_json::from_str::<Markdown>(json).is_err(),
                "forme objet acceptee alors que le contrat est une chaine : {json}"
            );
        }
    }

    #[test]
    fn la_lecture_refuse_tout_cequi_n_est_pas_une_chaine() {
        for json in ["1", "1.5", "true", "null", "[]"] {
            assert!(
                serde_json::from_str::<Markdown>(json).is_err(),
                "type non string accepte : {json}"
            );
        }
    }

    #[test]
    fn un_contenu_vide_survit_a_l_aller_retour() {
        // Un `.md` vide est un fichier reel. Il ne doit ni disparaitre, ni
        // etre confondu avec une valeur absente : c'est exactement ce que
        // ferait un test de veracite (`||`) au lieu d'un test de nullite
        // (`??`), qui n'aurait jamais de branche droite ici.
        let m = Markdown::nouveau(String::new());
        assert!(m.est_vide());
        let json = serde_json::to_string(&m).expect("serialisation");
        assert_eq!(json, "\"\"");
        let relu: Markdown = serde_json::from_str(&json).expect("deserialisation");
        assert!(relu.est_vide());
        assert_eq!(relu, m);
    }

    #[test]
    fn les_sauts_de_ligne_du_fichier_sont_conserves_tels_quels() {
        // Le `.md` d'origine est en CRLF. Rien dans le contrat ne normalise les
        // fins de ligne, donc rien ici ne le fait.
        let m = Markdown::nouveau("ligne1\r\nligne2\nligne3");
        let json = serde_json::to_string(&m).expect("serialisation");
        let relu: Markdown = serde_json::from_str(&json).expect("deserialisation");
        assert_eq!(relu.as_str(), "ligne1\r\nligne2\nligne3");
    }

    #[test]
    fn un_contenu_multibyte_survit_octet_pour_octet() {
        let source = "cafe\u{e9} \u{1f600} \u{4e2d}\u{6587} fin";
        let m = Markdown::nouveau(source);
        let json = serde_json::to_string(&m).expect("serialisation");
        let relu: Markdown = serde_json::from_str(&json).expect("deserialisation");
        assert_eq!(relu.as_str(), source);
        // Les compteurs portent sur des scalaires, pas sur des octets.
        assert_eq!(source.len(), 21);
        assert_eq!(source.chars().count(), 13);
    }

    #[test]
    #[should_panic(expected = "char boundary")]
    fn tronquer_sur_un_index_octet_au_milieu_d_un_caractere_panique() {
        // Risque connu du lot, ici montre et non evite : dans "cafe\u{e9}", le
        // `e` accentue occupe les octets 3..5, donc l'index 4 est au milieu.
        // C'est pour cela que l'API expose `as_str` et rien qui coupe.
        let m = Markdown::nouveau("cafe\u{e9}");
        let _ = &m.as_str()[..4];
    }

    #[test]
    fn une_frontiere_de_caractere_est_elle_un_index_octet_valide() {
        // Le pendant utile du test precedent : `char_indices` renvoie des
        // positions sur lesquelles le slicing est sur.
        let m = Markdown::nouveau("cafe\u{e9} \u{1f600}");
        let positions: Vec<usize> = m.as_str().char_indices().map(|(i, _)| i).collect();
        assert_eq!(positions, vec![0, 1, 2, 3, 6, 7]);
        for position in positions {
            assert!(m.as_str().is_char_boundary(position));
        }
    }

    #[test]
    fn as_str_et_into_string_rendent_la_meme_chaine() {
        let m = Markdown::nouveau("contenu");
        assert_eq!(m.as_str(), "contenu");
        assert_eq!(m.clone().into_string(), "contenu");
    }
}
