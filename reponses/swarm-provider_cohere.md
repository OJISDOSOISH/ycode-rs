# swarm-provider_cohere

===DEBUT===
fichier : src/swarm/provider_cohere.rs
source  : packages/core/src/plugin/provider/cohere.ts
taille  : 14359 octets (335 lignes)
tests   : 10

CONFIANCE : haute
POINT FAIBLE : le seul ecart assume est le `import("@ai-sdk/cohere")`, remplace par un trait `CohereModule` que l'hote fournit (Rust n'a pas d'import dynamique) ; et `options` est un `BTreeMap<String, Value>` alors que le TS y glisse un `fetch` qui n'a pas de forme JSON. A verifier aussi : `id` = "cohere" (sans scope) contre paquet = "@ai-sdk/cohere" (avec scope), les deux chaines doivent rester distinctes.
A VERIFIER : (1) que les 4 noms de champs de l'evenement sont bien `model`, `package`, `options`, `sdk`, en minuscules, sans camelCase ni variante ; (2) que le filtre est une egalite de chaine exacte et non un prefixe -- "@ai-sdk/Cohere", "@ai-sdk/cohere-extra" et "" doivent etre ignores ; (3) que le gestionnaire mute l'evenement et renvoie `()`, comme l'original, et qu'un `sdk` deja present survit a un evenement ignore.
===FIN===
