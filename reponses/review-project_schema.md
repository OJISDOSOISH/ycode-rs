# review-project_schema (directe, hors verificateurs)

Source : project/schema.ts (16 lignes). Cible : src/swarm/project_schema.rs.

## VERDICT : APPROUVE

## Points verifies OK
1. Zero duplication : ProjectId/AbsolutePath en `pub use` depuis
   session::schema, avec renvoi explicite a la passe C. L'agent a lu les
   decisions, pas seulement la source.
2. Deux Vcs distingues (Literal "git" nu vs objet {type,store}) avec
   consequence concrete citee (watcher `location.vcs?.type`). Point de
   verification le plus important, traite.
3. r#type + enum unitaire VcsType au lieu de String : fidelite de
   decodage (refus "mercurial"/"Git"), teste.
4. GLOBAL_PROJECT_ID en &str const (String non-const) : justification
   correcte. Vide survit ici, disparait dans project.ts:67 : distinction
   par call-site explicite et exacte.
5. Tests : 7, sens ecriture + lecture, casse, absence de cle, roundtrip.

## Nits non bloquants
- Depend de session::schema (fichier Kilo non pousse) : integre ensemble,
  pas separement. Rappel pour le push.
- Compilation reelle : CI.
