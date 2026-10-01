# swarm-event_sql

===DEBUT===
fichier : src/swarm/event_sql.rs
source  : packages/core/src/event/sql.ts
taille  : 42690 octets (1050 lignes)
tests   : 31

CONFIANCE : haute

POINT FAIBLE : le point faible de la vague 3 est TRANCHE, et tranche contre sa
proposition. J ai reecrit `event.id` en `nullable: true`, et j ai verifie la
question dans le depot plutot que de l assumer : NON, `primaryKey()` n implique
pas `NOT NULL` chez Drizzle, et le schema deja en service le prouve. Preuves, les
trois etant des artefacts produits par le projet lui-meme :
  1. `packages/core/src/database/schema.gen.ts:81` cree `event` avec
     `` `id` text PRIMARY KEY ``, SANS `NOT NULL`. Et ligne 74, `event_sequence`
     avec `` `aggregate_id` text PRIMARY KEY ``, alors que la source ecrit bien
     `.notNull().primaryKey()`. Les DEUX formes donnent le meme DDL : Drizzle
     retire le `NOT NULL` quand il emet `PRIMARY KEY`. Meme DDL dans la migration
     d origine `src/database/migration/20260323234822_events.ts:16`.
  2. L instantane drizzle-kit
     `packages/opencode/migration/20260511173437_session-metadata/snapshot.json`
     enregistre `"notNull": false` pour `event.id` ET pour
     `event_sequence.aggregate_id`. Donc dans le modele Drizzle les deux drapeaux
     sont independants et le second l emporte a la generation.
  3. Dans SQLite, une `PRIMARY KEY` non `INTEGER` d une table a `rowid` n
     implique PAS `NOT NULL` (deviation documentee, conservee pour compatibilite).
     Donc `event.id` ET `event_sequence.aggregate_id` peuvent physiquement valoir
     `NULL`. Ce point la n est PAS verifie en execution, faute de pilote sur la
     machine : c est le seul bout de doctrine non empirique du fichier.

A VERIFIER dans cet ordre :
  - Que `nullable` veuille dire "la source n ecrit pas `notNull()`", rien de plus.
    Le fichier le dit partout, et deux fonctions distinctes separent les deux
    questions : `ColumnDef::not_null_declares` (ce que la source ecrit) et
    `ColumnDef::admet_null_dans_sqlite` (ce que la base autorise). Elles ne
    different que pour `event.id` (false puis true). Tests :
    `la_cle_primaire_admet_null_a_la_base_meme_quand_la_source_l_interdit`,
    `la_seule_colonne_sans_not_null_ecrit_dans_l_evenement_est_son_identifiant`,
    `les_colonnes_admettant_null_a_la_base_sont_plus_nombreuses_que_cellules_sans_not_null_ecrit`.
    Si la relecture prefere l inverse (une seule notion de nullabilite), alors il
    faut supprimer une des deux fonctions, pas melanger les deux.
  - QUE REVOILENT LES AUTRES FICHIERS DU LOT. Neuf autres sources portent la meme
    forme `text().$type<X>().primaryKey()` sans `notNull()` : `session/sql.ts`
    (5 fois), `project/sql.ts`, `permission/sql.ts`, `credential/sql.ts`,
    `account/sql.ts`, `control-plane/workspace.sql.ts`, `share/sql.ts`,
    `data-migration.sql.ts`. Toutes generent le meme `text PRIMARY KEY` sans
    `NOT NULL`. Le resultat ci-dessus leur est reutilisable tel quel, et un agent
    qui a mis `nullable: false` par reflexe sur sa cle primaire a probablement
    la meme erreur que la vague 3.
  - Aucun piege de noms sur ce fichier : tous les noms de colonnes sont en
    snake_case, donc aucun `#[serde(rename)]` sauf `type`, mot cle Rust, porte
    par `event_type` avec `#[serde(rename = "type")]`. Deux tests verrouillent le
    nom : `le_champ_type_sort_du_cote_json_sous_le_nom_type_et_pas_sous_event_type`
    et `les_deux_lignes_serialisees_n_exposent_exactement_les_noms_de_leurs_colonnes`
    (les cles y sont triees avant comparaison, sinon le test depend du drapeau
    `preserve_order` de serde_json).
  - Le piege `?` contre `??` : la source ne contient NI ternaire NI coalescent
    (25 lignes, deux declarations, zero expression). Le fichier le dit et
    n invente rien. Le seul voisin est la chaine vide, qui releve d une autre
    distinction : `notNull()` interdit `NULL`, pas `""`. Une ligne aux chaines
    vides reste donc recevable, test
    `une_chaine_vide_reste_valide_car_l_absence_de_valeur_est_interdite_mais_pas_la_chaine_vide`,
    et `un_owner_id_vide_survit_a_la_serialization_et_ne_devient_pas_absent`.
  - L ordre des colonnes des deux index n est pas cosmetique :
    `event_aggregate_seq_idx` = (aggregate_id, seq) UNIQUE,
    `event_aggregate_type_seq_idx` = (aggregate_id, type, seq) non unique. Les
    deux noms et les deux listes se relisent dans `schema.gen.ts:239-240`.

AUTRE :
  - AUCUNE COMPILE. Ni cargo, ni cargo check, ni cargo test, ni rustc, ni rustup.
    Relecture manuelle, plus un controle mecanique de l equilibre des
    accolades et de l absence d octet non ASCII (0 octet non ASCII, 0 accent,
    0 guillemet typographique, 42690 octets).
  - Le `PartialEq` entre `&[&str]` et `&[&str; 2]` que la vague 3 signalait comme
    risque est corrige : les deux assertions d index comparent des tranches,
    `&["aggregate_id", "seq"][..]`, donc le trait existe sans hesiter.
  - Correction de la vague 3 : `nullable` ne vaut plus "interdit NULL" mais
    "la source n ecrit pas notNull()". Consequence sur les tests : l ancien
    `la_colonne_owner_id_est_la_seule_du_schema_a_admettre_l_absence_de_valeur`
    etait FAUX et a ete remplace. Idem la documentation ne pretendait plus que le
    piege vide/absence etait le piege `?` contre `??` du projet.
  - AUCUNE couche SQL fabriquee : pas de `CREATE TABLE`, pas de chaine de
    requete, pas de squelette de migration, aucune dependance ajoutee
    (`Cargo.toml` ne declare ni sqlx, ni rusqlite, ni diesel, ni libsql). Ce qui
    est porte : la declaration en donnees pures constantes (TABLE_EVENT_SEQUENCE,
    TABLE_EVENT, EVENT_INDEXES) et six fonctions pures qui executent sur une
    tranche de donnees les invariants que ces declarations imposent
    (`conflits_de_sequence`, `agregats_orphelins`,
    `evenements_survivants_a_la_suppression`, `positions_avec_donnee_invalide`,
    `sequences_sans_evenement`, plus les 6 acces d introspection des colonnes).
    Rappel de la vague 3, toujours vrai : ce sont MES fonctions, pas celles de
    la source, qui n en a aucune. Si la relecture juge que l index unique ne doit
    pas devenir une fonction, le fichier se reduit aux constantes et aux structs
    de lignes, et les tests de contrainte sont a supprimer avec elle.
  - `EventV2.ID` n est PAS defini ici : il vient de `packages/schema/src/event.ts`
    (chaine marquee, prefixe `evt_`), non porte par ce lot. `EventRow.id` est donc
    un `String`, et le fichier explique pourquoi ce n est PAS un `Option<String>`
    malgre `nullable: true` : le code applicatif ecrit toujours un identifiant,
    la colonne ne l impose pas.
  - Integration : `src/swarm/mod.rs` ligne 34 declare DEJA `pub mod event_sql;`.
    Le rapport de la vague 3 disait le contraire, c est faux aujourd hui. Je n ai
    touche ni a mod.rs, ni a lib.rs, ni a Cargo.toml, ni a aucun autre fichier.
  - Aucun commit, aucun push.
===FIN===
