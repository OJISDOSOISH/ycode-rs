# review-observability_shared

VERDICT: BUG

## Localisation
- Cote TS: opencode/packages/core/src/observability/shared.ts:1
  `export const runID = crypto.randomUUID().slice(0, 8)`
- Cote Rust: ycode-rs/src/swarm/observability_shared.rs:47-50 (imports), 53 (static RUN_ID), 59-61 (run_id), 67-72 (generer_run_id), 75-80 (nanosecondes_depuis_epoch), 41-45 (commentaire alea faux)
- Rapport: ycode-rs/reponses/swarm-observability_shared.md:28-34
- Preuve feature: ycode-rs/Cargo.toml:14 `uuid = { version = "1", features = ["v4", "v7", "serde"] }`
- Charte: ycode-rs/src/core/session/compaction.rs lu comme reference de style (consts en SCREAMING_SNAKE, fn en snake_case, docs en francais sans accents, tests ciblant pieges UTF-8 et seuils)

## BUG bloquant 1 - source d'entropie bricolee au lieu de Uuid::new_v4()
La source TS tire un UUID v4 via crypto.randomUUID(), donc 122 bits CSPRNG, puis garde les 8 premiers caracteres hex minuscules, soit 32 bits CSPRNG, avec tiret en position 9 donc jamais inclus.
Le portage fait :
`RandomState::new().build_hasher()` + `SystemTime::now()` en nanosecondes, puis `format!("{:08x}", hasher.finish() as u32)`.
Ce n'est pas un CSPRNG, c'est previsible (horloge lisible par tout processus) et melange par SipHash non crypto, avec cle jetable par appel. Cela ne respecte pas la consigne explicite : feature uuid/v4 disponible dans Cargo.toml, donc Uuid::new_v4() doit etre utilise.
Le rapport justifie l'ecart par "crate uuid n'active que v7 et serde" (lignes 28-34), ce qui est faux au vu de Cargo.toml:14 qui active bien v4, v7, serde. Le commentaire Rust lignes 41-45 reprend la meme erreur.
Correction attendue sans changer l'API OnceLock : garder `static RUN_ID: OnceLock<String>` + `pub fn run_id() -> &'static str`, et remplacer `generer_run_id` par tirage v4, par exemple `Uuid::new_v4().simple().to_string()[..8].to_string()` ou `Uuid::new_v4().to_string()[..8].to_string()`, avec test de format inchange.

## Points verifies OK
- Pattern OnceLock + fonction : OK. Rust interdit un const calcule, la fonction `run_id()` avec `get_or_init` reproduit bien la semantique "une seule valeur par processus, partagee partout". Le test pointeur identique lignes 118-122 le prouve.
- Format 8 hex minuscules sans tiret : OK en surface. `{:08x}` donne toujours 8 caracteres, padding par zeros a gauche, charset 0-9 a-f, sans tiret, coherent avec slice(0,8) d'un UUID canonique dont le tiret est en position 9. Tests lignes 88-108 couvrent longueur, charset, absence de tiret.
- Stabilite : OK. `run_id()` renvoie meme &str a chaque appel, test lignes 111-115.
- Absence de serde(rename) : OK. Aucun struct ni champ serialise dans ce module, donc rien a renommer. Remarque du rapport lignes 15-19 correcte sur ce point.
- Non panic : OK. `duration_since` avec `unwrap_or(0)`, pas d'index brut sur UUID, pas de coupure UTF-8 car sortie ASCII.
- Nom `run_id` vs `runID` : OK, convention Rust snake_case conforme a la charte compaction.rs, ecart d'API assume et documente.

## Nits non bloquants
- Commentaire lignes 20-25 sur forme canonique UUID : exact mais verbeux, a garder.
- Commentaire lignes 37-45 a corriger en meme temps que le BUG, sinon il restera faux apres fix.
- Test lignes 124-131 "seize generations donnent plus d'une valeur" : ne prouve pas le caractere aleatoire crypto, masque la faiblesse. A remplacer ou completer par test de format sur sortie v4 + test de stabilite du OnceLock deja present.
- Pas de test d'unicite inter processus : normal, hors scope unitaire.
- References a logging.ts et otlp.ts lignes 13-16 : non verifiees ici, mais coherentes avec l'usage attendu d'un run id global. Si ces fichiers existent, verifier qu'ils appellent bien `run_id()` et ne dupliquent pas leur propre tirage.

## Conclusion
Refuser en l'etat. Un seul changement bloque l'approbation : basculer `generer_run_id` sur `Uuid::new_v4()` et corriger doc + rapport. Le reste (OnceLock, format, stabilite, absence de rename) est conforme.
