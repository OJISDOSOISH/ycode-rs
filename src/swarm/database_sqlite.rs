//! Portage Rust de `packages/core/src/database/sqlite.ts`.
//!
//! ## Ce que contient la source
//!
//! Huit lignes, deux services et un type opaque :
//!
//! ```ts
//! export type DrizzleClient = ReturnType<typeof drizzle>
//! export class Native extends Context.Service<Native, unknown>()("@opencode-ai/core/database/SqliteNative") {}
//! export class Drizzle extends Context.Service<Drizzle, DrizzleClient>()("@opencode-ai/core/database/SqliteDrizzle") {}
//! ```
//!
//! `Native` est le pilote brut (Bun ou Node selon la plateforme, voir
//! `sqlite.bun.ts` et `sqlite.node.ts`, portes ailleurs). `Drizzle` est le
//! client ORM construit au-dessus. Les deux layers qui les relient vivent dans
//! ces memes fichiers de plateforme, pas ici.
//!
//! ## Ce qui n'a pas de traduction ici, et pourquoi
//!
//! - `Context.Service` : le conteneur d'effets. Porte comme tag et marqueur,
//!   pas comme conteneur.
//! - `DrizzleClient` (`ReturnType<typeof drizzle>`) : un type infere du pilote
//!   `drizzle-orm/bun-sqlite`. Sans pilote ni ORM dans `Cargo.toml`, la seule
//!   chose honnete est de dire que le client est opaque (`ClientOpaco` ne
//!   retient que son origine), pas d'inventer ses methodes.
//! - `layer`, `nativeLayer`, `sqliteLayer`, `drizzleLayer` : ils appartiennent
//!   aux fichiers de plateforme, qui choisissent le pilote selon le runtime.

/// Tag du service natif, second argument de `Context.Service`.
pub const NATIVE_TAG: &str = "@opencode-ai/core/database/SqliteNative";

/// Tag du service Drizzle, second argument de `Context.Service`.
pub const DRIZZLE_TAG: &str = "@opencode-ai/core/database/SqliteDrizzle";

/// Les deux tags du module, dans l'ordre de la source (natif puis ORM).
pub const ETIQUETTES: [&str; 2] = [NATIVE_TAG, DRIZZLE_TAG];

/// Marqueur du pilote SQLite brut.
///
/// Transcription de `export class Native extends Context.Service<Native,
/// unknown>()` : le service natif expose `unknown`, donc le marqueur ne porte
/// aucune donnee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Natif;

/// Marqueur du client Drizzle construit au-dessus du pilote.
///
/// Transcription de `export class Drizzle extends
/// Context.Service<Drizzle, DrizzleClient>()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrizzleMarqueur;

/// Origine du client opaque, pour tracer quel pilote l'a construit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigineClient {
    /// Construit par `drizzle-orm/bun-sqlite` (voir `sqlite.bun.ts`).
    Bun,
    /// Construit par `drizzle-orm/node-sqlite` (voir `sqlite.node.ts`).
    Node,
}

/// Client Drizzle opaque.
///
/// La source le definit comme `ReturnType<typeof drizzle>` : un type infere
/// du pilote, dont ce module ne connait ni les methodes ni la forme. Le
/// marqueur retient seulement le pilote d'origine, ce qui suffit a router
/// sans inventer d'API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientOpaque {
    /// Pilote qui a construit ce client.
    pub origine: OrigineClient,
}

impl Natif {
    /// Tag du service natif.
    pub fn tag() -> &'static str {
        NATIVE_TAG
    }
}

impl DrizzleMarqueur {
    /// Tag du service Drizzle.
    pub fn tag() -> &'static str {
        DRIZZLE_TAG
    }
}

impl ClientOpaque {
    /// Construit un client opaque depuis son pilote d'origine.
    pub fn new(origine: OrigineClient) -> Self {
        Self { origine }
    }
}

/// Dit si un tag est l'un des deux services declares par la source.
pub fn est_etiquette_connue(tag: &str) -> bool {
    ETIQUETTES.contains(&tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_tags_sont_ceux_declares_dans_la_source() {
        assert_eq!(NATIVE_TAG, "@opencode-ai/core/database/SqliteNative");
        assert_eq!(DRIZZLE_TAG, "@opencode-ai/core/database/SqliteDrizzle");
        assert_eq!(ETIQUETTES, [NATIVE_TAG, DRIZZLE_TAG]);
    }

    #[test]
    fn le_natif_passe_avant_l_orm_comme_dans_la_source() {
        assert!(NATIVE_TAG.contains("SqliteNative"));
        assert!(DRIZZLE_TAG.contains("SqliteDrizzle"));
        assert_ne!(NATIVE_TAG, DRIZZLE_TAG);
    }

    #[test]
    fn les_marqueurs_exposent_leur_tag() {
        assert_eq!(Natif::tag(), NATIVE_TAG);
        assert_eq!(DrizzleMarqueur::tag(), DRIZZLE_TAG);
    }

    #[test]
    fn seules_les_deux_etiquettes_sont_connues() {
        assert!(est_etiquette_connue(NATIVE_TAG));
        assert!(est_etiquette_connue(DRIZZLE_TAG));
        assert!(!est_etiquette_connue("@opencode/v2/storage/Database"));
        assert!(!est_etiquette_connue(""));
    }

    #[test]
    fn le_client_opaque_retient_son_pilote_sans_inventer_d_api() {
        let bun = ClientOpaque::new(OrigineClient::Bun);
        let node = ClientOpaque::new(OrigineClient::Node);
        assert_eq!(bun.origine, OrigineClient::Bun);
        assert_eq!(node.origine, OrigineClient::Node);
        assert_ne!(bun, node);
    }
}
