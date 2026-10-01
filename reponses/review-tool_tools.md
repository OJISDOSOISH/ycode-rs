# review-tool_tools

VERDICT : APPROUVE

Cible TS : opencode/packages/core/src/tool/tools.ts (13 lignes reelles, 441 octets, verifie par powershell).
Cible Rust : ycode-rs/src/swarm/tool_tools.rs (239 lignes, 9 tests).
Rapport : ycode-rs/reponses/swarm-tool_tools.md.
Charte lue : ycode-rs/src/core/session/compaction.rs (style, tests francophones, doc honnete sur les divergences).

## Points verifies OK

1. SERVICE_TAG exact.
   - TS ligne 13 : "@opencode/v2/Tools".
   - Rust ligne 45 : SERVICE_TAG = "@opencode/v2/Tools".
   - Test ligne 149-151 : assert_eq strict. OK.

2. Signature register.
   - TS lignes 7-9 : (tools: Readonly<Record<string, Tool.AnyTool>>) => Effect<void, RegistrationError, Scope>.
   - Rust lignes 99-107 : fn register(&mut self, tools: &BTreeMap<String, AnyTool>) -> Result<(), RegistrationError>.
   - void -> (), canal erreur conserve, lecture seule conserve par &BTreeMap. L ecart Scope est documente lignes 19-24 et 96-98. OK car tools.ts ne fait que nommer le contrat, l implementation Scope est dans registry.ts hors lot.

3. AnyTool minimal.
   - TS : Definition<any, any> defini dans tool/tool.ts lignes 20-27, objet gele vide avec fantomes _Input/_Output.
   - Rust lignes 61-62 : pub struct AnyTool; vide. Effacement des fantomes correct, aucun champ donc aucun rename JSON a poser. OK.

4. RegistrationError champs exacts.
   - TS : tool/tool.ts lignes 31-34, TaggedErrorClass("Tool.RegistrationError", { name: String, message: String }).
   - Rust lignes 69-76 : champs name: String, message: String, sans majuscule interne, donc sans serde rename. Constante REGISTRATION_ERROR_TAG ligne 53 = "Tool.RegistrationError", methode tag() lignes 85-87, test lignes 221-227. Pas de frontiere JSON dans tools.ts, donc absence de _tag serialise admise et avouee dans le rapport. OK.

5. Reexport ligne 1 TS (export * as Tools from "./tools", auto reexport circulaire).
   - Rust lignes 34-36 : ignore a raison, le module joue deja ce role. OK.

6. Pieges JS passes en revue, rien a porter dans ce fichier.
   - ? vs ?? : aucune occurrence dans tools.ts.
   - truthiness : aucune condition dans tools.ts.
   - UTF-16 length / .length / slice : aucun.
   - f64/i64/u64 : aucun nombre dans tools.ts.
   - Variantes perdues : aucune union dans tools.ts.
   - Insertion order : seul piege pertinent, traite en nit 2 ci-dessous, documente par le code et un test dedie.

7. Tests couvrant les pieges du lot.
   - 9 tests : tag exact, catalogue vide accepte, un outil, ordre alphabetique documente, nom vide conserve, cumul de deux appels, erreur remontee sans ecriture, conservation nom/message/tag, usage via Box<dyn Service>. Suffisant pour 13 lignes de contrat.

## Nits non bloquants

1. Metadonnees du rapport fausses : annonce "10456 octets (240 lignes)". Reel : 441 octets, 10 lignes (13 avec fin de fichier). Confusion probable avec tool.ts (162 lignes) ou registry.ts (147 lignes). A corriger dans le rapport, sans impact sur le code.

2. Ordre de parcours : BTreeMap impose l ordre alphabetique, objet JS l ordre d insertion. Rust le documente lignes 103-105 et test lignes 168-186. Effet de bord : si deux noms invalides, TS (registry.ts ligne 88, Object.entries + validateName) signale le premier insere, Rust signalerait le premier alphabetique. Hors lot (validation dans tool/tool.ts), accepte car assume et teste.

3. Scope.Scope sans traduction mecanique : reporte sur &mut self de l appelant. Perte reelle du finalizer de desenregistrement (registry.ts lignes 94-102), mais ce code est dans registry.ts hors lot, pas dans tools.ts. Documentation honnete, accepte.

4. Display invente ligne 70 ("enregistrement d outil refuse : ...") et derives en plus (Copy, Default, Hash sur AnyTool ligne 61). Aucun equivalent TS, mais sans effet sur le contrat. Accepte.

5. Doublon futur signale a raison par le rapport : AnyTool / RegistrationError devront disparaitre au profit du portage de tool/tool.ts quand il atterrira. Ne pas modifier src/tool.rs dans ce lot. OK.
