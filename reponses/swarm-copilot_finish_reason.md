# swarm-copilot_finish_reason

===DEBUT===
fichier : src/swarm/copilot_finish_reason.rs
source  : packages/core/src/github-copilot/chat/map-openai-compatible-finish-reason.ts
taille  : 7478 octets
tests   : 11

CONFIANCE : haute
POINT FAIBLE : l'union exacte `LanguageModelV3FinishReason["unified"]` du SDK est
inverifiable ici, car `@ai-sdk/provider` n'est pas installee dans node_modules ;
j'ai donc limite l'enum aux 5 valeurs que la fonction produit reellement et j'ai
volontairement laisse dehors la variante `"error"` que le SDK contient
probablement, ce qui pourrait ne pas correspondre a l'ensemble reel.
A VERIFIER : (1) que la valeur de retour attendue par le code appelant est bien
un enum et non un `&'static str` -- si un fichier voisin veut la chaine directe,
il devra passer par `.as_str()` ; (2) le piege de la valeur de RETOUR, que j'ai
couvert par `une_raison_inconnue_devient_other_sans_la_recopier` mais qui
n'est verifiable qu'a la relecture : le `default` du `switch` renvoie bien la
litterale `"other"`, jamais la valeur d'entree, et c'est la seule chose que le
portage pouvait gotten de travers ; (3) le code n'a jamais ete compile, aucun
toolchain Rust n'existe sur ce poste, il n'y a donc aucune preuve qu'il passe
la CI ; (4) `mod.rs` declarait deja `pub mod copilot_finish_reason;` ligne 20,
je n'y ai donc rien touche et aucune declaration manquante n'est a faire.

NOTE : `serde` et `serde_json` sont deja dans `Cargo.toml`, rien a ajouter.
===FIN===
