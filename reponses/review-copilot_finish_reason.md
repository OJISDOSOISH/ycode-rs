# review-copilot_finish_reason

Source : opencode/packages/core/src/github-copilot/chat/map-openai-compatible-finish-reason.ts (19 lignes)
Cible : ycode-rs/src/swarm/copilot_finish_reason.rs (223 lignes, 11 tests)
Rapport : ycode-rs/reponses/swarm-copilot_finish_reason.md
Charte : ycode-rs/src/core/session/compaction.rs

VERDICT : APPROUVE

La fonction pure est integralement portee. Les 5 cas TS ont chacun leur equivalent Rust, le defaut renvoie bien la litterale "other", et les 11 tests couvrent les pieges deduits de la source. Aucun BUG bloquant.

## Points verifies OK

1. Mapping exhaustif, aucune variante perdue :
   - TS lignes 7-8 `stop` -> `stop` / Rust ligne 66 + enum ligne 34 + as_str ligne 50. Test `une_raison_stop_donne_stop`.
   - TS lignes 9-10 `length` -> `length` / Rust ligne 67 + enum ligne 36 + as_str ligne 51. Test `une_raison_length_donne_length`.
   - TS lignes 11-12 `content_filter` -> `content-filter` / Rust ligne 68 + enum lignes 38-39 + as_str ligne 52. Tiret bas en entree, tiret en sortie, fige et exact. Test `content_filter_devient_content_filter_avec_un_tiret`.
   - TS lignes 13-15 `function_call` + `tool_calls` (fallthrough) -> `tool-calls` / Rust ligne 69 `Some("function_call") | Some("tool_calls")`. Equivalent exact du fallthrough. Test `function_call_et_tool_calls_donnent_la_meme_valeur` incluant as_str `tool-calls`.
   - TS lignes 16-17 `default` -> litterale `"other"` / Rust ligne 72 `_ => Other`. Jamais de recopie de l entree. Tests `une_raison_inconnue_devient_other_sans_la_recopier` avec `assert_ne!` anti-recopie.
2. Entree `string | null | undefined` (TS ligne 4) -> `Option<&str>` (Rust lignes 62-64) : fusion de `null` et `undefined` en `None` licite car les deux tombent dans le meme `default` en TS. Test `une_raison_absente_devient_other`.
3. Chaine vide : `Some("")` ne matche aucun motif et tombe sur `_`, comme `""` tombe sur `default` dans le switch TS. Test `une_chaine_vide_devient_other`.
4. Egalite stricte sensible a la casse : le switch JS compare sans normalisation, le match Rust sur motifs de chaines fait pareil. Tests `la_casse_compte_comme_en_javascript` (`STOP`, `Content_Filter` -> other) et `une_variante_sans_espace_ne_devient_pas_stop` (`" stop"` -> other).
5. Renames JSON exacts, piege ID/Id/majuscules absent ici mais verifie : `stop`, `length`, `content-filter`, `tool-calls`, `other` (Rust lignes 34-43). Coherents avec `as_str` lignes 49-55. Tests aller `les_chaines_emises_sont_bien_celles_du_sdk` et retour `chaque_variante_se_relit_depuis_sa_chaine` avec controle de coherence `as_str` vs serde a chaque variante.
6. Aucun piege operateur JS dans la source : pas de `||`, `??`, `!`, coercition numerique, truthiness, `typeof` ou arithmetique. Seul un switch strict, reproduit par match. Pas de risque `NaN`, `0`, `""` falsy detourne.
7. Types : retour TS `LanguageModelV3FinishReason["unified"]` (union de chaines) rendu par enum + `as_str()` + Serialize/Deserialize. L appelant TS utilise la valeur comme chaine (`unified:` dans `openai-compatible-chat-language-model.ts` lignes 277 et 457), et `.as_str()` fournit cette chaine en Rust. Entree schema zod `z.string().nullish()` (lignes 772 et 809 du meme fichier) confirme `Option<&str>`.
8. Charte `compaction.rs` respecte : docs de tete en francais expliquant la fidelite, fonctions et tests en francais snake_case, commentaires pieges en clair, comme exige.
9. Declaration module presente dans `ycode-rs/src/swarm/mod.rs`, rien a ajouter.

## Nits non bloquants

- Variante `error` absente de l enum : correct pour cette fonction qui ne la produit jamais (le rapport l auto-signale en POINT FAIBLE). Ne l ajouter que le jour ou l enum SDK complet est porte ailleurs. En attendant, une deserialisation d un hypothetique `"error"` venu d ailleurs echouerait, ce qui est hors contrat de cette fonction.
- Retour enum vs `&'static str` : le rapport le signale deja a juste titre. L appelant Rust futur devra passer par `.as_str()` s il veut la chaine directe. L API actuelle le permet, donc non bloquant.
- Ligne annoncee dans le rapport pour `mod.rs` : le rapport dit ligne 20, constate ligne 21. Simple decalage, sans effet.
- Compilation non prouvee sur le poste auteur (aucun toolchain, comme pour tout le lot swarm d apres `mod.rs` lignes 11-13). Le code se lit comme compilable avec `serde` + `serde_json` deja au manifest, mais la preuve reste la CI.
