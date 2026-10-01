# review-util_lazy

VERDICT : APPROUVE

## Sources relues en entier

- TS : `opencode/packages/core/src/util/lazy.ts` lignes 1-11 (11 lignes, fonction `lazy<T>(fn)` avec `value: T | undefined` + `loaded: boolean`, `loaded = true` avant `fn()`).
- Charte : `ycode-rs/src/core/session/compaction.rs` lignes 1-171 (style de reference : doc en tete, constantes documentees, tests en francais sans accents, pas de JSON invente).
- Rapport : `ycode-rs/reponses/swarm-util_lazy.md` lignes 1-48.
- Rust : `ycode-rs/src/swarm/util_lazy.rs` lignes 1-215 (struct `Lazy<T>` + `lazy()` + 7 tests).
- Conteneur : `ycode-rs/src/swarm/mod.rs` ligne 55 (`pub mod util_lazy;` present, non modifie).
- Usages reels TS : `opencode/packages/core/src/filesystem/watcher.ts` lignes 26-36 (`lazy(() => ... | undefined)` avec `catch { return }`) et `opencode/packages/core/src/pty.ts` ligne 18 (`lazy(() => import("#pty"))`).

## Points verifies OK

1. Memoization : un seul appel au calcul. TS `if (loaded) return value` / Rust `src/swarm/util_lazy.rs:89-99` (`if !self.loaded` + `init.take()`). Test `les_appels_suivants_ne_relancent_pas_le_calcul` (compteur == 1) OK.
2. Ordre `loaded = true` AVANT `fn()`. TS ligne 7-8 / Rust lignes 91-95 (`self.loaded = true` avant `init()`). Test `un_calcul_qui_panique_ne_permet_pas_de_relancer_la_lecture` verifie `is_loaded()` vrai apres panique OK. Fidele a la source.
3. Separation `loaded` / `value`, pas de conflation avec `Option`. TS `T | undefined` + `boolean` separes / Rust lignes 47-56 (`value: Option<T>`, `loaded: bool` separes, `init: Option<Box<...>>` consomme). Test `un_resultat_absent_du_type_utilisateur_est_bien_memorise` (`T = Option<String>`, calcul renvoie `None`, compteur == 1) OK. Couvre exactement le cas `watcher.ts:26-36` qui renvoie `undefined` apres `catch` sans relancer le `require` a chaque appel.
4. JSON : aucun echange JSON dans ce module, aucun champ a renommer. Piege ID/Id/majuscules : sans objet. Rapport le dit, confirme.
5. Variantes perdues : aucune enum, aucun union a part `T | undefined` traite en point 3. `z.lazy` dans `file-search.ts` est un autre `lazy` (zod), hors scope.
6. Operateurs JS pieges : seul `if (loaded)` (booleen strict), pas de `?`, pas de `??`, pas de truthiness sur objet, pas de `||` avec defaut. Traduction en `bool` Rust exacte.
7. UTF-16 length : sans objet, aucune chaine manipulee, aucun `.length`, aucun `slice`.
8. Insertion order : sans objet, aucune Map / objet / cle parcourue.
9. Types numeriques : sans objet, aucun `f64` / `i64` / `u64`, `T` generique sans borne des deux cotes. Pas de conversion ni d overflow possible.
10. Concurrence : source sans promesse ni verrou, portage sans `Mutex` / `OnceCell` / `RefCell`, documente en tete lignes 7-13. Choix correct, rien a ajouter.
11. Retour par reference : TS renvoie le meme objet, Rust renvoie `&T` (lignes 89-104) sans borne `T: Clone` ajoutee. Test `la_valeur_memorisee_reste_la_meme_adresse` (egalite de pointeurs) OK. Equivalent honnete du partage JS.
12. Construction paresseuse : test `creer_un_lazy_ne_lance_pas_le_calcul` OK. Independance des instances : test `deux_lazy_independants_ne_partagent_pas_leir_calcul` OK.
13. Divergence panique vs `undefined` : documentee (tete + `# Panique` lignes 80-88) et testee. En JS apres echec `loaded` reste vrai et lecture suivante renvoie `undefined` ; en Rust `T` ne peut pas valoir "aucune valeur", la panique explicite est la seule traduction honnete. Pas un BUG, ecart inevitable et assume. Meme analyse pour reentree recursive (pret documente ligne 84-85).
14. Hygiene : fichier Rust 8566 octets, 0 octet > 126 verifie par scan local. `mod.rs` deja a jour. Aucun commit / push (lecture seule + tests).
15. Compilation locale : `cargo test swarm::util_lazy` non executable ici (`link.exe` manquant sur cible msvc, pas de toolchain). Verdict base sur relecture ligne a ligne + usages, pas sur execution.

## Nits non bloquants

- `Box<dyn FnOnce() -> T + 'static>` (lignes 50, 62) impose `'static` : une fermeture capturant une reference non statique est refusee alors que le TS accepte tout. Alternative : struct generique `Lazy<T, F>` ou `Box<dyn ... + 'a>` avec lifetime. Ne bloque pas les usages connus (`watcher`, `pty` sont statiques), a noter pour futurs appelants.
- `get(&mut self) -> &T` restreint la surface par rapport a la fermeture JS appelable de partout (rapport le signale). Appelant en methode `&self` devra choisir sa strategie. Pour types `Copy`, penser a dereferencer `*l.get()` ; pour autres, `.clone()` explicite. Pas de `Clone` impose, c est le bon compromis.
- `is_loaded()` est une API ajoutee, absente du TS. Additive et utile aux tests, pas de risque.
- Coquilles texte sans impact : ligne 37 `legitimately` (anglais dans doc francophone), test ligne 190 `leir` pour `leur`. Orthographe a corriger a l occasion.
- Doc ligne 15 `en revanchevolontairement` : espace manquante. Pure forme.

## Conclusion

Fidele a `lazy.ts:1-11`, ordre et memoization preserves, cas `None` / `undefined` couvert, divergences Rust inevitables documentees et testees. Rien a corriger avant integration.
