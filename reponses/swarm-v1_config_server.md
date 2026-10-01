# swarm-v1_config_server

===DEBUT===
fichier : src/swarm/v1_config_server.rs
source  : packages/core/src/v1/config/server.ts
taille  : 25595 octets
tests   : 13

VAGUE : 5 (amelioration d'un fichier deja porte en vague 3 par kilo-swarm3)

CONFIANCE : haute

POINT FAIBLE : `port` est un `Option<u64>` qui ACCEPTE `0` a la deserialization,
alors que `PositiveInt` en TypeScript vaut exactement
`Schema.Int.check(Schema.isGreaterThan(0))` (`packages/schema/src/schema.ts`
ligne 3, verifie) et le refuse au parsing. C'est la seule divergence de
comportement reelle qui reste dans ce fichier. Je ne l'ai **pas** corrigee, sur
volonte : `est_positif` existe et suffit a l'appelant, mais l'appliquer ici
seul divergerait de `config_tool_output.rs` qui a fait le meme choix pour le
meme type. Le corriger proprement demande de faire de `PositiveInt` un newtype
valide au decodage, dans le module qui le definit, donc dans un fichier qui
n'est pas le mien. A trancher une fois pour tout le lot.

A VERIFIER (par ordre) :
1. **Les 5 cles JSON.** `port`, `hostname`, `mdns`, `mdnsDomain`, `cors`. Les
   cinq portent un `#[serde(rename = ...)]` EXPLICITE, relus un par un contre
   la source. `mdnsDomain` est le SEUL nom camelCase du fichier et le piege
   principal. Deux tests(nowveaux en vague 5) verrouillent le nom, voir plus bas.
2. **Syntaxe jamais typee.** Aucun compilateur n'a ete utilise sur ce poste, la
   regle l'interdit. Points suspects que j'ai traites a la lecture :
   `survit_si_null::<u64>(None)` a un turbofish explicite parce que le `None` seul
   ne donne rien a compiler sur un generique, et les deux appels
   `assert_eq!(server.cors, survit_si_null(Some(Vec::new())))` /
   `assert_eq!(server.hostname, survit_si_null(Some(String::new())))` dont le
   type de `T` est推断 par le membre gauche.
3. **`.or(None)`** dans `survit_si_null` est la seule mecanique un peu atypique du
   fichier. Elle est equivalente a la fonction identite sur `Option<T>` et ne
   demande aucun `T: Copy`, c'est delibere.
4. INTEGRATION : **resolu depuis la vague 3.** `pub mod v1_config_server;` est
   bien la ligne 80 de `src/swarm/mod.rs`, verifie. Le rapport de vague 3
   signalait ce point comme bloquant, il ne l'est plus. Je n'ai pas touche
   `mod.rs`.

CE QUE J'AI CHANGE PAR RAPPORT A LA VAGUE 3
---------------------------------------------
Le fichier de la vague 3 etait bon et je l'ai **conserve tel quel** partout ou
il etait juste. Le struct, les cinq `rename`, `SCHEMA_IDENTIFIER`, le
reexport de `PositiveInt`, et les 9 tests d'origine sont inchanges. J'ai fait
cinq ajouts, tous orientes sur les deux pieges de la mission :

1. **Piege 1, le test de refus explicite de `mdns_domain`.** Il manquait. Le
   test existant verifiait que `mdns_domain` n'apparait pas dans le JSON
   **produit**, ce qui ne prouve rien sur la **lecture**. J'ai ajoute
   `le_nom_snake_case_mdns_domain_est_refuse_a_la_lecture` : il relit
   `{"mdns_domain":...}` et exige que le champ reste `None`, puis essaie les
   trois autres fautives (`mdnsdomain`, `MdnsDomain`, `mdns_DOMAIN`) et exige
   le meme resultat, puis un **temoin** qui exige que `mdnsDomain` fonctionne
   toujours. Le temoin est indispensable : sans lui, un `rename` errone vers
   un nom qui n'existe pas passerait pour un refus correct. Raison de fond :
   `#[serde(rename)]` REMPLACE le nom du champ, il ne s'y ajoute pas, donc la
   forme snake_case est une propriete inconnue ignoree, pas une erreur de
   parsing. Si le `rename` disparait un jour, ce test echoue, ce que le JSON
   ne revelait pas.

2. **Piege 2, deux fonctions distinctes au lieu d'une.** Le fichier de la
   vague 3 disait "zero ternaire, zero coalescent" ce qui est vrai mais ne
   prouve rien. J'ai ecrit les DEUX jugements de valeurs sous deux formes
   distinctes, jamais fusionnees, dans le module de tests :
   `disparait_si_falsy` (veracite, famille `?`) et `survit_si_null` (nullite,
   famille `??`). Le test
   `les_deux_familles_de_jugement_ne_donnent_pas_le_meme_resultat` montre
   qu'elles **divergent sur `Some(0)`** (l'entier equivalent de `Some("")`) et
   qu'elles convergent sur une valeur pleine et sur une absence. Ce sont des
   fonctions de TEST, pas du code porte : ce module n'expose ni l'une ni
   l'autre, donc la regle "ne fabrique rien" sur l'API publique est respectee.
   Le module documente que la source n'a que la famille nullite.

3. **Le piege de veracite sur un booleen, pas seulement sur une chaine.**
   `false` est falsy en JavaScript au même titre que `""`, et c'est
   exactement le cas que le test de la vague 3 ne couvrait pas.
   `le_decodage_de_la_configuration_suit_la_famille_nullite` verifie que
   `{"mdns":false}` survit en `Some(false)` et se reserialise en
   `{"mdns":false}`, et que le champ absent reste absent.

4. **Une deuxieme divergence documentee, la borne de `Schema.Int`.**
   `Schema.Int` est un entier JavaScript, donc plafonne par
   `Number.MAX_SAFE_INTEGER` (2^53 - 1) ; `u64` monte a 2^64 - 1. Le test
   `un_entier_au_dela_du_plafond_javascript_est_accepte` fixe le comportement
   Rust, sans arrondi ni troncature. **Le comportement exact du schema `effect`
   cote TypeScript n'a pas pu etre verifie** : la bibliotheque `effect` n'est
   pas installee sur cette machine, ce que d'autres agents du lot ont signale
   pour `@ai-sdk/provider` aussi.

5. **Doc : le renvoi croise vers `config_tool_output.rs` a ete complete** avec
   le chemin de definition reel de `PositiveInt` (`packages/schema/src/schema.ts`
   ligne 3) et avec l'explication du POURQUOI on ne branche pas `est_positif`
   sur la deserialisation, ce qui etait le point le plus utile du rapport de
   vague 3 et qui n'etait que dans le rapport, pas dans le fichier.

AUTRES POINTS UTILES
---------------------
- 0 caractere non ASCII, verifie octet par octet (les 4 accents et tirets cadratins
  de ma premiere passe ont ete corriges ; les autres agents du lot seemblent
  viser le meme zero).
- `cors` est `Vec<String>` et **non** `BTreeSet` : la source ecrit
  `Schema.mutable(Schema.Array(...))`, le mot `mutable` est explicite, donc ce
  n'est pas un `readonly` et l'ordre de lecture compte pour une liste de
  domaines. C'est le SEUL point ou je m'ecarte de la table de conversion du
  contexte. Deux tests verrouillent l'ordre inverse.
- Aucune mecanique reseau fabriquee. Le fichier parle d'un port, d'un hote et de
  mDNS mais **n'ouvre rien** : ce sont des valeurs de configuration, pas des
  actions. Ni `TcpListener`, ni `UdpSocket`, ni thread d'annonce. Le binding,
  la resolution de nom et le traitement CORS se font ailleurs dans le
  TypeScript.
- `opencode.local` n'apparait QUE dans le texte d'une description, entre
  parentheses. Ce n'est ni une constante ni un defaut du schema et la source ne
  l'applique nulle part : je ne l'ai donc pas declaree, et **aucun des cinq
  champs n'a de defaut**. Le defaut reel est choisi par le code qui lit cette
  configuration, qui n'est pas dans ce fichier.
- `export * as ConfigServerV1 from "./server"` est un reexport d'espace de noms
  vers le fichier lui-meme : code mort, rien a porter. C'est un motif recurrent
  dans opencode, plusieurs agents du lot l'ont signale sur d'autres fichiers.
- C'est un `Schema.Struct` et non une `Schema.Class` : une seule forme de
  donnees, aucune methode, aucune instance. `SCHEMA_IDENTIFIER = "ServerConfig"`
  est conserve pour la lisibilite de la correspondance, comme `config_command.rs`.
- Consequence `Schema.optional` connue, identique au reste du lot : Serde lit
  `null` comme `None`, la ou le schema TypeScript distingue `null` de l'absence.
  Invisible pour du JSON produit par le TypeScript, visible sur une saisie
  manuelle.
- Conséquence `Schema.Struct` : les proprietes inconnues sont tolerees, donc
  pas de `deny_unknown_fields`. C'est ce qui rend le test de refus du point 1
  forme `assert_eq!(champ, None)` et non `assert!(...is_err())`.
- Aucun commit, aucun push. Je n'ai touche que mon fichier.

===FIN===
