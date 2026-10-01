# swarm-config_watcher

===DEBUT===
fichier : src/swarm/config_watcher.rs
source  : packages/core/src/config/watcher.ts
taille  : 13224 octets (310 lignes)
tests   : 13

CONFIANCE : haute
POINT FAIBLE : le seul doute serieux est l'egalite de `null` avec l'absence de
  cle. J'ai suppose que `Schema.optional` (sans `exact: true`) traite `null`
  comme `undefined`, donc que `{"ignore": null}` doit se lire comme une absence
  -- et le test `une_valeur_nulle_vaut_une_absence_de_motif` verrouille ce
  choix. Si la version d'Effect utilisee traite `null` comme une erreur, ce
  test est faux et la lecture doit devenir stricte. Verifiez aussi que
  `Vec<String>` (et non `BTreeSet`) est bien voulu : j'ai argues que l'ordre
  compte parce que le consumer concatene `[...Ignore.PATTERNS, ...config,
  ...protecteds(...)]` avant `subscribe`, mais personne n'a verifie que
  l'ordre a une incidence observable chez parcel.
A VERIFIER :
  1. `null` contre absence (point faible principal, cf. plus haut).
  2. le nom de champ JSON : `ignore`, tout en minuscules, aucun
     `#[serde(rename)]` pose. Le test `le_nom_du_champ_json_est_ignore`
     verifie la casse et l'absence de pluriel, mais c'est le seul nom en jeu.
  3. `#[derive(Default)]` sur `Info` : ajout Rust pur, sans equivalent TS.
     Harmonique, mais c'est la seule API de mon fichier qui n'a pas de
     contrepartie exacte dans la source.

CE QUE LE FICHIER NE FAIT PAS, ET C'EST VOLONTAIRE :
  la source fait 7 lignes : un auto-reexport (`export * as ConfigWatcher from
  "./watcher"`, sans equivalent en Rust, donc rien a ecrire) et UN champ,
  `ignore: Array<string> | undefined`. Aucun mecanisme de surveillance
  n'existe dans la source : ni thread, ni abonnement, ni emission
  d'evenement. Tout cela vit dans `packages/core/src/filesystem/watcher.ts`
  via `@parcel/watcher`, et `Cargo.toml` ne declare aucun crate de
  surveillance (ni `notify`, ni `watchexec`). Je n'ai donc FABRIQUE AUCUNE
  mecanique de surveillance : ni boucle, ni canal, ni matching de motif. Le
  seul point de contact est `ignore_or_empty()`, qui reproduit le `?? []` de
  `filesystem/watcher.ts` ligne 108, et rien de plus.

A SIGNALER (je n'y touche pas) : `pub mod config_watcher;` est deja present
dans `src/swarm/mod.rs` ligne 20. Aucune declaration a ajouter.
===FIN===
