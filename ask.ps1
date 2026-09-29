<#
.SYNOPSIS
  Sous-agent de portage, via appel direct a Nemotron.

.DESCRIPTION
  Kilo ne sait pas router vers NVIDIA : il repond "Model not found" en
  proposant le nom exact qu on lui a donne. L appel direct fonctionne, donc on
  contourne. Ce lanceur est donc la seule voie qui marche.

  Sortie : le texte brut du modele, ecrit dans un fichier de sortie plutot que
  dans la sortie standard, parce que la reponse peut faire plusieurs kilo-octets
  et que PowerShell la tronquerait.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Brief,
    [ValidateSet("super", "ultra")][string]$Model = "super",
    [Parameter(Mandatory)][string]$OutFile,
    [int]$MaxTokens = 16000,
    [int]$TimeoutSec = 1500
)

$ErrorActionPreference = 'Stop'

$models = @{
    super = "nvidia/nemotron-3-super-120b-a12b"
    ultra = "nvidia/nemotron-3-ultra-550b-a55b"
}

if (-not (Test-Path $Brief)) { throw "Brief introuvable : $Brief" }
if (-not $env:NVIDIA_API_KEY)   { throw "NVIDIA_API_KEY absent" }

$brief = Get-Content $Brief -Raw -Encoding utf8

$payload = @{
    model       = $models[$Model]
    messages    = @(
        @{ role = "system"; content = "Tu es un sous-agent de portage TypeScript vers Rust. Tu produis uniquement du code Rust, sans explication." }
        @{ role = "user";   content = $brief }
    )
    max_tokens  = $MaxTokens
    temperature = 0.15
} | ConvertTo-Json -Depth 10

$payloadFile = [System.IO.Path]::GetTempFileName()
$outTmp      = [System.IO.Path]::GetTempFileName()
$errTmp      = [System.IO.Path]::GetTempFileName()
Set-Content -Path $payloadFile -Value $payload -Encoding utf8 -NoNewline

Write-Host "==> $($models[$Model])" -ForegroundColor Cyan

$proc = Start-Process -FilePath "curl.exe" -ArgumentList @(
    "-sS", "--max-time", "$TimeoutSec",
    "-X", "POST",
    "https://integrate.api.nvidia.com/v1/chat/completions",
    "-H", "Authorization: Bearer $env:NVIDIA_API_KEY",
    "-H", "Content-Type: application/json",
    "--data-binary", "@$payloadFile"
) -NoNewWindow -PassThru -RedirectStandardOutput $outTmp -RedirectStandardError $errTmp

if (-not $proc.WaitForExit($TimeoutSec * 1000)) {
    Write-Host "    delai depasse, arret" -ForegroundColor Yellow
    $proc.Kill(); $proc.WaitForExit(5000) | Out-Null
    Remove-Item $payloadFile, $outTmp, $errTmp -Force -ErrorAction SilentlyContinue
    exit 124
}

$raw = Get-Content $outTmp -Raw -ErrorAction SilentlyContinue
Remove-Item $payloadFile, $outTmp, $errTmp -Force -ErrorAction SilentlyContinue

if (-not $raw) { Write-Host "reponse vide" -ForegroundColor Red; exit 1 }

$json = $raw | ConvertFrom-Json
if ($json.error) { Write-Host ("ERREUR API : " + $json.error.message) -ForegroundColor Red; exit 1 }

$text = $json.choices[0].message.content
Set-Content -Path $OutFile -Value $text -Encoding utf8

if ($json.usage) {
    Write-Host ("    {0} tokens sortants" -f $json.usage.completion_tokens) -ForegroundColor DarkGray
}
Write-Host ("    ecrit : {0} ({1:N0} car.)" -f $OutFile, $text.Length) -ForegroundColor Green
