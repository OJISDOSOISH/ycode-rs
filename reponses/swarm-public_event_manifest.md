# swarm-public_event_manifest

===DEBUT===
fichier : src/swarm/public_event_manifest.rs
source  : packages/core/src/public-event-manifest.ts
taille  : 15528 octets
tests   : 9

CONFIANCE : haute
POINT FAIBLE : le fichier source fait 7 lignes et ne contient que deux reexports
  (`EventManifest.ServerDefinitions` et `Event.latest(Definitions)`) ; j'ai donc
  transcrit la table de cles en remontant la chaine jusqu'a
  `packages/schema/src/event-manifest.ts` puis dans les 30 fichiers de
  definitions. Si un autre agent porte deja un de ces fichiers schema, nous
  avons la meme table en double. Second point faible : les compteurs sont
  recalcules depuis la source et ne correspondent plus au test TypeScript
  `packages/schema/test/event-manifest.test.ts`, qui est anterieur (il annonce
  55 / 85 / 32, la source donne 58 / 88 / 35, soit exactement 3 de moins sur
  chacun, ce qui s'explique par 3 definitions `session.next.*` ajoutees depuis).
A VERIFIER : (1) les 88 cles et leur ordre, en priorité les 32 `session.next.*`
  et les 7 `session v1` durables qui forment le bloc 4..42, identical dans
  `DEFINITIONS` et `SERVER_DEFINITIONS` ; (2) que `session.next.step.ended` ET
  `session.next.step.failed` sont bien en version 2 et seuls eux, ils
  utilisent `stepSettlementOptions` en amont ; (3) l'index 58 :
  `DEFINITIONS[58] == "pty.deleted"` depend du fait que `feature` (14 entrees)
  commence a l'index 48, apres 43 de foundation + 3 v1 live + 2 installation.
  A signaler au relecteur : le fichier n'est declare dans aucun `mod.rs`, je n'y
  touche pas.
===FIN===
