# Revue lot oc-1 : 7/7 APPROUVES (avec 2 micro-correctifs integres)

Integres en src/core/ + cables dans core/mod.rs. Extraction exacte des
rapports (script, zero recopie), 100 % ASCII verifie par execution.

## command (command.ts:1-64 relu) : APPROUVE
Defaut {name, template:""}, name force, get/list/remove fideles. Model.Ref
canonique confirme (repond au POINT FAIBLE). Divergence documentee : list()
trie (BTreeMap) alors que Map garde l'insertion ; intentionnel (deterministe,
pas d'IndexMap au catalogue), meme precedent que config_plugin.

## open (open.ts:1-8 relu) : APPROUVE
Message d'erreur exact, WHATWG approche honnetement (scheme+autorite+/ final),
UrlOpener comme point de branchement. Pas de crate url : confirme voulu.

## provider : APPROUVE
Union taggee type, integrationID verrouille, empty() miroir des statics.
Signale pour C : duplique la def schema (fusion a la passe C).

## model : APPROUVE
parse() exact jusqu'aux cas "" et multi-slash, empty_info miroir, Cost f64 /
Limit i64 correctement discrimines. Signale pour C : ProviderId et ModelRef
dupliques (provider.rs, schema.rs, session_message.rs).

## credential : APPROUVE
Pieges ?/?? tous corrects (stored falsy, label ?? default, garde update).
Tagged union + integrationID/methodID verifies. Divergences documentees :
ID_COUNTER local vs ascending TS (unicite OK, format a recaler sur
util_identifier plus tard), colonnes SQL non lues ignorees.

## image : APPROUVE avec 2 micro-correctifs appliques a l'integration
- message ResizerUnavailable invente -> rend le tag (jamais d'invention).
- ImageLimits : renames camelCase du contrat adaptateur ajoutes.
exceeds_limits/check_size gardes comme helpers documentes (controle reel
dans photon non porte, dit explicitement).

## global : APPROUVE
xdg-basedir vide->defaut confirme (vrai comportement du paquet), ??
vs veracite testes, required_dirs ordre+contenu, Flock/app-node hors
perimetre a raison.

## Doublons connus -> passe C (Kilo)
ProviderId x2, ModelRef x3-4, CredentialId x2 (integration_connection).
Controle les listera ; C les unifie dans src/schema/.
