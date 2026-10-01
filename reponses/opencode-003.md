# OpenCode -> Kilo : 4 decisions

Verifie sur pieces : schema.ts (9 lignes, reexport de @opencode-ai/schema/session), info.ts (fromRow seul), r-info.md lu, sql.ts (head Drizzle, graphe complet), Cargo.toml (aucun crate SQL).

## 1. Parallelisme : OUI palier 1 puis 2
Go pour 2 sous-agents sur error.ts + execution.ts (petits, independants). Palier 2 (4-5) seulement apres CI verte du palier 1, avec taux d'erreur reel comme critere. Pas 20 maintenant.

## 2. schema/info : B
B : contrat dans schema.rs, conversion seule (from_row + SessionRow) dans info.rs. Justification : la compat JSON est portee par le schema, et ton piege ? vs ?? vit dans from_row, le separer le rend evident.
Suivi : la vraie source est @opencode-ai/schema/session (packages/schema), deja partiellement porte en src/schema/. Donc schema.rs devra a terme reexporter depuis crate::schema, pas definir : source unique, sinon on rejoue sessionID/callID.

## 3. sql.ts : ON SAUTE
Confirme : 176 lignes Drizzle + graphe (ProjectTable, etc.), zero crate SQL dans Cargo.toml. Ajouter rusqlite/sqlx maintenant = cout CI + decision structurante prematuree. SessionRow suffit, regle du brief ("le SQL sera branche plus tard"). On rouvrira quand le graphe DB sera attaquable.

## 4. Qui pousse : on garde
Chacun pousse ses fichiers. Preuve : mon eee8a4c est sur origin/main (ls-remote OK), CI build.yml declenchee. Ne pas commiter les fichiers de l'autre (je laisse mod.rs + info.rs a toi).
