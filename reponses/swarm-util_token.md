# swarm-util_token

===DEBUT===
fichier : src/swarm/util_token.rs
source  : packages/core/src/util/token.ts
taille  : 5659 octets
tests   : 9

CONFIANCE : haute
POINT FAIBLE : j'ai compile la longueur de la chaine en **codes units UTF-16** (`input.encode_utf16().count()`) pour reproduire exactement `input.length` du TypeScript, et non en octets (`str::len()`) ni en points de code (`chars().count()`). Si la relecture juge que le portage attend un compte plus simple, c'est le point a trancher en premier : un emoji compte pour 2 ici alors qu'il ferait 1 avec `chars().count()` et 4 avec `len()`. Deuxieme doute, moindre : j'ai rendu `estimate` en `i64` signe (et non `usize`) pour que l'appelant de `session/compaction.ts` qui fait `Token.estimate(x) > context - summaryOutput` ne risque pas une underflow quand `context - summaryOutput` est negatif.
A VERIFIER : (1) que le comptage UTF-16 est bien le comportement attendu et que `mod.rs` declare bien `pub mod util_token;` -- je ne l'ai PAS ajoute, c'est au main de le faire ; (2) le type de retour `i64` face aux 4 appelants dans `session/compaction.ts` lignes 83, 149, 190 et 238 ; (3) qu'aucun appelant n'attend un `f64`.

AUTRES NOTES :
- PAS de cryptographie dans ce fichier. Ni signature, ni hachage, ni comparaison de secret. Le "token" est un jeton de contexte (unite de texte du modele), pas une credencial. La seule operation est `Math.max(0, Math.round(input.length / 4))`. Rien n'a ete affaibli ni invente.
- La ligne 1 de la source, `export * as Token from "./token"`, est un reexport de soi-meme : le chemin vise est le module lui-meme et il n'existe aucun autre fichier `token` sous `util/` (verifie par glob, seul `util/token.ts` existe). Rust n'a pas d'equivalent de cet alias de namespace, donc la ligne n'a pas de traduction et les appelants font `util_token::estimate`. Documente en tete de fichier.
- `CHARS_PER_TOKEN` est restee **privee**, comme dans la source ou elle n'est pas exportee.
- Piege `?` / `??` : sans objet ici, aucun operateur de ce type dans le fichier.
- Piege des noms de champs (`sessionID`, `projectID`...) : sans objet, le fichier n'a aucun struct, aucun enum, aucune serialisation. Aucun `serde(rename)` n'est pose, et il n'en faut aucun.
- Comportement deroule de la source, pine par les tests : `length` 0 et 1 donnent 0 ; `length` 2 et 3 donnent deja 1 (0.5 et 0.75 montent) ; `length` 4 et 5 donnent 1 ; `length` 6 et 7 donnent 2 ; `length` 8 et 9 donnent 2. Seule la longueur 0 et 1 rendent 0.
- `Math.round` (montee des moities) et `f64::round` (ecart au zero) ne different que sur les moities negatives, impossibles ici puisque la longueur est toujours >= 0. Le `Math.max(0, ...)` est reporte tel quel meme s'il est defensif.
- Source de 5 lignes, aucune dependance externe, aucun import. Aucun `serde` n'est utilise, le fichier compile donc meme sans la feature serde.
- Non compile : aucun toolchain Rust sur ce poste, conformement a la consigne. Relecture syntaxique seulement.
- Fichier 100 % ASCII verifie (0 octet > 127) : les caracteres accentues et l'emoji des tests sont ecrits en sequences d'echappement `\u{00e9}` et `\u{1F600}`, pas en litteraux.
===FIN===
