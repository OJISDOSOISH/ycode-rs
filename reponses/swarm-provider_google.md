# swarm-provider_google

===DEBUT===
fichier : src/swarm/provider_google.rs
source  : packages/core/src/plugin/provider/google.ts
taille  : 13100 octets
tests   : 7

CONFIANCE : haute
POINT FAIBLE : j'ai transforme `evt.sdk = mod.createGoogleGenerativeAI(evt.options)` en une **trace d'appel** (`SdkFactoryCall { factory, options }`) plutot qu'en un SDK, parce qu'un appel a du code JavaScript externe n'a pas d'equivalent Rust ; si la relecture attend un vrai mecanisme d'appel, c'est la que ca casse. Second point, plus mesurable : `options` est un `BTreeMap`, donc l'ordre des cles JSON devient alphabetique alors qu'un objet JavaScript garde l'ordre d'insertion. Les donnees sont identiques, mais une comparaison de chaine JSON entre le TS et le Rust peut differer.
A VERIFIER : (1) que le champ s'appelle bien `package` en clair (ni `r#package`, ni camelCase, et pas de `#[serde(rename)]` oublie) et que le test `les_noms_de_champs_serialises_sont_ceux_du_typescript` couvre les 4 cles et l'absence de `sdk` ; (2) que la fabrique est bien `createGoogleGenerativeAI` avec `AI` en majuscules, deux fois dans le fichier (constante et assertion) ; (3) que le `if evt.package != AI_SDK_PACKAGE` reste une egalite stricte et n'a pas derive en `starts_with` ; (4) la syntaxe Rust, jamais compilee sur ce poste (bloc ```text du doc comment, `serde_json::Map::new()`, `Value::get("providerID")`).
===FIN===
