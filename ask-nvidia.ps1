<#
.SYNOPSIS
  Sous-agent de portage, via appel direct a l'API NVIDIA.

.DESCRIPTION
  Kilo ne sait pas router vers les modeles NVIDIA : il repond "Model not found"
  en proposant lui-meme le nom qu on lui a donne. C est un bug de routage, pas un
  quota. L appel direct fonctionne (verifie le 2026-09-29), donc on contourne.

  Un seul appel par execution : on donne le brief, on recupere la reponse.
  Pas de boucle d outils : ce sous-agent n ecrit rien sur le disque, il propose
  le contenu, et c est l agent principal qui decide de l ecrire. Cela evite
  qu un modele produise un fichier casse sans controle.

.PARAMETER Brief
  Chemin du fichier de consignes.

.PARAMETER Model
  Modele NVIDIA. Seuls deux repondent : nemotron-3-super-120b-a12b et
  nemotron-3-ultra-550b-a55b. Les autres renvoient 404 sur ce compte.

.EXAMPLE
  .\ask-nvidia.ps1 -Brief .\BRIEF-PERM.md -Model super
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Brief,
    [ValidateSet("super", "ultra")][string]$Model = "super",
    [int]$MaxTokens = 8000,
    [int]$TimeoutSec = 900
)

$ErrorActionPreference = 'Stop'

$models = @{
    super = "nvidia/nemotron-3-super-120b-a12b"
    ultra = "nvidia/nemotron-3-ultra-550b-a55b"
}

if (-not (Test-Path $Brief)) { Write-Error "Brief introuvable : $Brief" }
if (-not $env:NVIDIA_API_KEY) { Write-Error "NVIDIA_API_KEY absent" }

$briefText = Get-Content $Brief -Raw -Encoding utf8

$payload = @{
    model       = $models[$Model]
    messages    = @(
        @{ role = "system"; content = "Tu es un sous-agent de portage TypeScript vers Rust. Tu reponds uniquement en Rust, sans commentaire superflu." }
        @{ role = "user";   content = $briefText }
    )
    max_tokens  = $MaxTokens
    temperature = 0.2
} | ConvertTo-Json -Depth 8

Write-Host "==> $($models[$Model])" -ForegroundColor Cyan

$outFile = [System.IO.Path]::GetTempFileName()
$errFile = [System.IO.Path]::GetTempFileName()

# PowerShell ne transmet pas fiablement un JSON long en argument de processus
# natif : on ecrit le corps dans un fichier et curl le lit avec --data-binary.
$payloadFile = [System.IO.Path]::GetTempFileName()
Set-Content -Path $payloadFile -Value $payload -Encoding utf8 -NoNewline

$proc = Start-Process -FilePath "curl.exe" -ArgumentList @(
    "-sS", "--max-time", "$TimeoutSec",
    "-X", "POST",
    "https://integrate.api.nvidia.com/v1/chat/completions",
    "-H", "Authorization: Bearer $env:NVIDIA_API_KEY",
    "-H", "Content-Type: application/json",
    "--data-binary", "@$payloadFile"
) -NoNewWindow -PassThru -RedirectStandardOutput $outFile -RedirectStandardError $errFile

$finished = $proc.WaitForExit($TimeoutSec * 1000)
if (-not $finished) { Write-Host "delai depasse" -ForegroundColor Yellow; $proc.Kill() }

$raw = Get-Content $outFile -Raw -ErrorAction SilentlyContinue
Remove-Item $outFile, $errFile, $payloadFile -Force -ErrorAction SilentlyContinue

if (-not $raw) { Write-Error "reponse vide" }

$json = $raw | ConvertFrom-Json
$text = $json.choices[0].message.content

if ($json.usage) {
    Write-Host ("    tokens : {0} entrants, {1} sortants" -f $json.usage.prompt_tokens, $json.usage.completion_tokens) -ForegroundColor DarkGray
}

Write-Host ""
Write-Host $text
