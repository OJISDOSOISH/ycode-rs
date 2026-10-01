# swarm-util_lazy

===DEBUT===
fichier : src/swarm/util_lazy.rs
source  : packages/core/src/util/lazy.ts
taille  : 8566 octets
tests   : 7

CONFIANCE : haute
POINT FAIBLE : l'API rend `&T` avec `&mut self` (choix idiomatique, mais la source rend une valeur appelable depuis partout) ; et le calcul qui echoue entre en panique la, ou JavaScript renvoie silencieusement `undefined`. Les deux divergences sont documentees dans l'entete du fichier, mais c'est la premiere qui peut generer des echecs de compilation chez les appelants futurs, pas chez moi.
A VERIFIER : (1) que `src/swarm/mod.rs` contient bien `pub mod util_lazy;` (je ne l'ai pas touche, mais je l'ai lu et il y est) ; (2) que le portage de `&mut self` + `&T` est accepte par la relecture comme equivalent du `() => T` TypeScript, ou qu'il faut revenir a un clone explicite ; (3) que la divergence sur le cas "le calcul echoue" est bien toleree.
===FIN===

## Detail pour la relecture

**Ce que fait la source (11 lignes).** Une fonction `lazy<T>(fn)` renvoie une fermeture
qui retient `value: T | undefined` et `loaded: boolean`. Au premier appel elle met
`loaded = true`, appelle `fn()`, et renvoie. Aux suivants elle renvoie `value` tel quel.

**Concurrence : il n'y en a pas dans la source.** Pas de promesse, pas de verrou,
pas d'`await`. Juste deux variables capturees. Je n'ai donc ajoute ni `Mutex`,
ni `OnceCell`, ni `RefCell`, ni aucun interior mutability. Le fichier le dit
explicitement en tete de module.

Ce qui n'a pas de traduction Rust, et que j'ai dit dans le fichier :
- le partage implicite de la fermeture mutable : remplace par `&mut self`, donc
  un appelant en methode `&self` ne peut pas l'utiliser sans strategie de sa part ;
- l'absence de borne de copie : `get` renvoie `&T`, ce qui evite d'ajouter un
  `T: Clone` inexistant dans la source.

**Fidelite.** `value` et `loaded` restent deux champs separes, comme dans la
source. Je n'ai pas utilise `Option<T>` comme temoin, sinon `Lazy<Option<T>>`
dont le calcul renvoie `None` aurait relance le calcul a chaque appel ; c'est
exactement ce que couvre le test `un_resultat_absent_du_type_utilisateur_est_bien_memorise`.
L'ordre est respecte aussi : `loaded` passe a `true` **avant** l'appel du calcul.

**Divergence assumee.** Si le calcul leve une panique, l'etat est
`loaded = true` / `value = None` ; la lecture suivante entre en panique. En
JavaScript elle renverrait `undefined` silencieusement. Rendre `T` sans valeur
n'a pas d'equivalent honnete en Rust : je prefere une panne explicite. Le cas
est documente dans le `# Panique` de `get` et couvert par un test.

**Aucun nom de champ a renommer** : ce fichier n'echange rien en JSON, il n'y a
donc aucun piege `camelCase` ici.

**Controles faits** : 100 % ASCII verifie (scan sur le fichier, 0 caractere
> 126), aucun accent, aucun guillemet typographique. `mod.rs` contient deja
`pub mod util_lazy;` (non modifie). Aucun commit, aucun push.
