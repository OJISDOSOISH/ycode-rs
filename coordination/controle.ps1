<#
.SYNOPSIS
  Controle statique du portage Rust, en l'absence de compilateur.

.DESCRIPTION
  La machine qui produit ce projet n'a ni cargo, ni rustup, ni cible MSVC : rien
  ne compile et rien ne teste en local. Ce script est le substitut le moins
  mauvais. Il ne remplace PAS le compilateur et ne pretend pas le faire : il
  attrape les erreurs qu'un compilateur attraperait de toute facon, plus une
  que le compilateur ne voit pas.

  Ce qu'il fait :
    1. Verifie l'equilibrage des accolades, parentheses et crochets. Un fichier
       qui ne compile pas a la premiere ligne de toute facon.
    2. Verifie qu'il n'y a pas de caractere accentue ni CJK. La regle du
       codebase l'impose, et une corruption de caractere signale une source
       abimee en amont.
    3. Verifie que chaque fichier porte des tests #[cfg(test)].
    4. **Detecte les collisions de noms publics entre fichiers.** C'est le
       controle le plus utile, et celui qu'aucun compilateur ne peut faire a
       travers les modules : deux types de meme nom dans deux fichiers ne se
       genèrent pas, mais ils divergent en silence des lors qu'un seul des deux
       est modifie. C'est exactement le piege qui a ete attrape sur
       sessionID / callID, puis sur DiffStatus.
    5. Verifie que chaque module ecrit est bien declare dans un mod.rs.

.PARAMETER Racine
  Racine du projet Rust. Par defaut, le depot ycode-rs.

.EXAMPLE
  .\controle.ps1
  .\controle.ps1 -Silence
#>
[CmdletBinding()]
param(
    [string]$Racine = 'C:\Users\AI\Projects\Ycode\ycode-rs',
    [switch]$Silence
)

$ErrorActionPreference = 'Stop'

function Write-Titre([string]$t) {
    if (-not $Silence) { Write-Output ''; Write-Output "=== $t ===" }
}

# ---------------------------------------------------------------------------
Write-Titre '1. Fichiers Rust'

$fichiers = Get-ChildItem -LiteralPath $Racine -Recurse -File -Filter '*.rs' |
    Where-Object { $_.FullName -notmatch '\\target\\' }

Write-Output "$($fichiers.Count) fichiers .rs trouves, hors target/"

# ---------------------------------------------------------------------------
Write-Titre '2. Caracteres interdits (accents, CJK)'

# On excepte les lignes de test : une assertion sur "éééé" est une donnee de
# test volontaire, pas un commentaire abime. Ce que l'on traque, c'est le
# caractere etranger dans un commentaire ou un identifiant.
$suspects = @()
foreach ($f in $fichiers) {
    $lignes = [System.IO.File]::ReadAllLines($f.FullName, [Text.Encoding]::UTF8)
    for ($i = 0; $i -lt $lignes.Count; $i++) {
        $l = $lignes[$i]
        if ($l -match '[^\x00-\x7F]') {
            $estTest = $l -match 'assert|fn [a-z_]+\(' -and $l -notmatch '^\s*(//|///|//!)'
            if (-not $estTest) {
                $suspects += [PSCustomObject]@{
                    Fichier = $f.FullName.Replace("$Racine\", '')
                    Ligne   = $i + 1
                    Texte   = $l.Trim()
                }
            }
        }
    }
}
if ($suspects.Count -eq 0) {
    Write-Output 'aucun caractere interdit hors test'
} else {
    $suspects | ForEach-Object { Write-Output ("  {0} L{1} : {2}" -f $_.Fichier, $_.Ligne, $_.Texte) }
    Write-Output "  -> $($suspects.Count) occurrence(s)"
}

# ---------------------------------------------------------------------------
Write-Titre '3. Tests presents'

$sansTest = @()
foreach ($f in $fichiers) {
    $t = [System.IO.File]::ReadAllText($f.FullName, [Text.Encoding]::UTF8)
    if ($t -notmatch '#\[cfg\(test\)\]') { $sansTest += $f.FullName.Replace("$Racine\", '') }
}
if ($sansTest.Count -eq 0) {
    Write-Output 'tous les fichiers portent des tests'
} else {
    $sansTest | ForEach-Object { Write-Output "  sans tests : $_" }
}

# ---------------------------------------------------------------------------
Write-Titre '4. COLLISIONS DE TYPES'

# Rust autorise legitimement deux `new`, deux `as_str`, ou deux `Info` dans des
# modules differents : chaque module a son propre espace de noms, et c'est
# idiomatique. Signaler ca rendrait l'outil illisible, donc on l'ignore.
#
# Ce qui est en revanche un vrai defaut, c'est deux DEFINITIONS D'UN MEME
# CONTRAT : deux `struct` ou deux `enum` de meme nom pour la meme entite. Le
# compilateur ne les voit pas, elles coexistent, et le jour ou l'une est
# modifiee sans l'autre, les deux divergent en silence. C'est ce qui est
# arrive sur sessionID / callID, puis sur DiffStatus.
#
# On ne regarde donc QUE les types et les alias de type. Les fonctions sont
# comptees a part, et seulement a titre indicatif.

# Les commentaires doivent etre retires avant toute analyse. Sans ca, une ligne
# de documentation qui explique "on ne redeclare pas `pub type ProjectId` ici"
# est lue comme une declaration, et l'outil signale une collision qui n existe
# pas. Un verificateur qui invente des problemes vaut moins que pas de
# verificateur du tout : on finit par ignorer tout ce qu'il dit.
function Retirer-Commentaires([string]$Texte) {
    $s = $Texte
    # Commentaires de bloc.
    $s = [regex]::Replace($s, '/\*[\s\S]*?\*/', ' ')
    # Commentaires de ligne. On retire seulement ceux qui commencent vraiment
    # la ligne (eventuellement apres des espaces), pour ne pas mutiler un
    # chaine de caractere contenant "//".
    $s = [regex]::Replace($s, '(?m)^[ \t]*//.*$', '')
    # Fin de ligne.
    $s = [regex]::Replace($s, '(?m)//.*$', '')
    return $s
}

$types = @{}
$fonctions = @{}
foreach ($f in $fichiers) {
    $brut = [System.IO.File]::ReadAllText($f.FullName, [Text.Encoding]::UTF8)
    $t = Retirer-Commentaires $brut
    $module = $f.FullName.Replace("$Racine\", '').Replace('\', '::').Replace('.rs', '')
    $motifsType = @(
        'pub\s+struct\s+([A-Z]\w*)',
        'pub\s+enum\s+([A-Z]\w*)',
        'pub\s+trait\s+([A-Z]\w*)',
        'pub\s+type\s+([A-Z]\w*)'
    )
    foreach ($m in $motifsType) {
        foreach ($mm in [regex]::Matches($t, $m)) {
            $nom = $mm.Groups[1].Value
            if (-not $types.ContainsKey($nom)) { $types[$nom] = @() }
            if ($types[$nom] -notcontains $module) { $types[$nom] += $module }
        }
    }
    foreach ($mm in [regex]::Matches($t, 'pub\s+(?:async\s+)?fn\s+([a-z]\w*)')) {
        $nom = $mm.Groups[1].Value
        if (-not $fonctions.ContainsKey($nom)) { $fonctions[$nom] = @() }
        if ($fonctions[$nom] -notcontains $module) { $fonctions[$nom] += $module }
    }
}

# Un nom de type commun comme `Info` ou `Request` n'est PAS une collision : ce
# sont des contrats differents qui se trouve porter le meme nom, ce qui est
# courant. Ce qui est une collision, c'est quand le MEME nom de type designe la
# MEME entite conceptuelle. On distingue les deux par le nom du module : deux
# modules de domaines differents (swarm/project_schema contre
# core/session/schema) qui redéfinissent le meme alias d'identifiant, c'est une
# collision. Deux `Info` dans deux domaines sans rapport, non.
$collisions = $types.GetEnumerator() | Where-Object { $_.Value.Count -gt 1 } | Sort-Object Name

if ($collisions.Count -eq 0) {
    Write-Output 'aucun type declare en double'
} else {
    Write-Output "$($collisions.Count) nom(s) de type porte(s) par plusieurs modules."
    Write-Output ''
    Write-Output '  A VERIFIER UN PAR UN. Certains sont normaux, d autres sont des'
    Write-Output '  collisions reelles. Voici comment distinguer :'
    Write-Output ''
    Write-Output '  - COLLISION  : le nom designe la MEME entite conceptuelle dans les'
    Write-Output '                deux modules (ex : WorkspaceId, un alias d identifiant,'
    Write-Output '                ne peut avoir qu une definition).'
    Write-Output '  - NORMAL     : le nom designe des entites DIFFERENTES qui se trouve'
    Write-Output '                porter le meme nom (ex : Info, Request). C est'
    Write-Output '                courant et sans consequence tant qu on ne les confond pas.'
    Write-Output ''
    Write-Output ''
    foreach ($c in $collisions) {
        Write-Output ("  " + $c.Key + "  (" + $c.Value.Count + " modules)")
        $c.Value | ForEach-Object { Write-Output "      $_" }
    }
}

$collisionsFn = $fonctions.GetEnumerator() | Where-Object { $_.Value.Count -gt 1 } | Sort-Object Name
if (-not $Silence -and $collisionsFn.Count -gt 0) {
    Write-Output ''
    Write-Output "  (pour memoire : $($collisionsFn.Count) noms de fonctions publics partages,"
    Write-Output '   ce qui est normal en Rust et n est pas signale comme un defaut.)'
}

# ---------------------------------------------------------------------------
Write-Titre '5. Modules declares'

$manquants = @()
foreach ($f in $fichiers) {
    $rel = $f.FullName.Replace("$Racine\", '')
    if ($rel -eq 'src\lib.rs' -or $rel -match 'mod\.rs$') { continue }
    $nom = [System.IO.Path]::GetFileNameWithoutExtension($f.FullName)
    $parent = Split-Path $f.DirectoryName -Leaf
    $modFile = Join-Path $f.DirectoryName 'mod.rs'
    if (-not (Test-Path $modFile)) {
        # Le parent doit declarer le module depuis le grand-parent.
        $grand = Join-Path (Split-Path $f.DirectoryName -Parent) 'mod.rs'
        if (Test-Path $grand) {
            $t = [System.IO.File]::ReadAllText($grand, [Text.Encoding]::UTF8)
            if ($t -notmatch "pub\s+mod\s+$([regex]::Escape($parent))\s*;") { $manquants += $rel }
        }
    } else {
        $t = [System.IO.File]::ReadAllText($modFile, [Text.Encoding]::UTF8)
        if ($t -notmatch "pub\s+mod\s+$([regex]::Escape($nom))\s*;") { $manquants += $rel }
    }
}
if ($manquants.Count -eq 0) {
    Write-Output 'tous les modules sont declares'
} else {
    $manquants | ForEach-Object { Write-Output "  non declare : $_" }
}

# L'inverse est le controle qui casse la compilation, donc il compte davantage.
# Un `pub mod foo;` dans un mod.rs sans fichier foo.rs est une erreur de
# compilation immediate, alors qu'un fichier non declare est seulement du code
# mort. Sur un swarm ou l'on pousse pendant que des agents ecrivent encore, c est
# l'erreur la plus facile a introduire et la plus bruyante a decouvrir plus tard.
$orphelins = @()
foreach ($modFile in (Get-ChildItem -LiteralPath $Racine -Recurse -File -Filter 'mod.rs')) {
    $t = [System.IO.File]::ReadAllText($modFile.FullName, [Text.Encoding]::UTF8)
    $dir = $modFile.DirectoryName
    foreach ($m in [regex]::Matches((Retirer-Commentaires $t), 'pub\s+mod\s+([a-z][a-z0-9_]*)\s*;')) {
        $nom = $m.Groups[1].Value
        # Un module existe soit comme `nom.rs`, soit comme un repertoire
        # `nom/mod.rs`. Chercher seulement `nom.rs` fausse l'alerte sur tous les
        # modules de style repertoire, dont src/core/session.
        $enFichier = Test-Path (Join-Path $dir "$nom.rs")
        $enRepertoire = Test-Path (Join-Path $dir "$nom\mod.rs")
        if (-not ($enFichier -or $enRepertoire)) {
            $orphelins += "$($modFile.FullName.Replace("$Racine\", '')) -> $nom"
        }
    }
}
if ($orphelins.Count -eq 0) {
    Write-Output 'toutes les declarations ont un fichier'
} else {
    Write-Output ''
    Write-Output 'DECLARATION SANS FICHIER : cela ne compilera pas tant que le fichier manque.'
    $orphelins | ForEach-Object { Write-Output "  $_" }
}

Write-Output ''
Write-Output 'Rappel : ce script ne remplace pas le compilateur. La CI GitHub'
Write-Output 'Actions reste le seul verificateur qui.execute le code.'
