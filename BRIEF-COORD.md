Tu es un sous-agent de portage. Mission : traduire UN fichier TypeScript en Rust.

LIS la source ENTIEREMENT avant d ecrire la moindre ligne.

REGLES ABSOLUES
- Tu ne modifies QUE le fichier cible. Aucun autre.
- `effect` / `Effect.fn` devient une fonction Rust pure. Pas d async si inutile.
- Drizzle / SQL : isole la logique metier dans une fonction pure qui prend une
  tranche de donnees. Le SQL sera branche plus tard par quelqu un d autre.
- Les `Schema.X` deviennent des structs derives Serialize / Deserialize.
- Les ADT a tag deviennent des `enum` avec `#[serde(tag = "...")]` et un
  `#[serde(rename = "...")]` EXPLICITE sur chaque variante.
- Tout champ camelCase du TS porte un `#[serde(rename = "camelCase")]`.
  C est l erreur d incompatibilite la plus frequente dans ce portage.
- Les optionnels du TS deviennent `Option<T>` avec
  `#[serde(skip_serializing_if = "Option::is_none")]`.
- Ecris au moins 3 tests #[cfg(test)] couvrant les cas limites que tu deduis :
  liste vide, un seul element, ordre inverse, valeurs par defaut.
- Commentaires et documentation en francais SANS ACCENTS (contrainte codebase).
- Si la source n est qu un export vide, ecris un fichier minimal qui l explique
  et ne fabrique rien. Ne comble pas les trous par imagination.

LIVRABLE : le fichier Rust, ecrit sur le disque. Pas de reponse textuelle.

---
SOURCE : C:\Users\AI\Projects\Ycode\opencode\packages\core\src\session\run-coordinator.ts
CIBLE   : C:\Users\AI\Projects\Ycode\ycode-rs\src\core\session\run_coordinator.rs
CHARTE  : C:\Users\AI\Projects\Ycode\ycode-rs\src\core\session\compaction.rs   (lis-le pour le style et le niveau de detail attendu)
