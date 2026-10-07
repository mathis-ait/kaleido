# Oracle externe de la légalisation (chantier A du PRD) : exporte les Pokémon rendus
# légaux par Kaleido en .pk4 à .pk7, puis les fait vérifier en lot par PKHeX.
#
#   powershell -ExecutionPolicy Bypass -File scripts\pkhex-check.ps1 [-Out <dossier>]
#
# Étape 1 (toujours) : `kaleido legal-dir` sur les corpus PKHeX (légaux et illégaux) et
# `kaleido legal-smogon` sur le corpus Smogon figé (crates/core/tests/data/smogon).
#
# Étape 2 (si un vérificateur PKHeX est disponible) : PKHeX n'a pas d'outil en ligne de
# commande officiel. Le script appelle l'exécutable désigné par la variable
# d'environnement PKHEX_CHECK avec la liste des fichiers ; il doit écrire une ligne
# « <chemin><TAB>Legal » ou « <chemin><TAB>Illegal<TAB><raison> » par fichier.
# Un tel outil tient en quelques lignes avec le paquet NuGet PKHeX.Core (GPLv3) :
#
#   dotnet new console -n PkhexCheck ; cd PkhexCheck ; dotnet add package PKHeX.Core
#   // Program.cs
#   using PKHeX.Core;
#   foreach (var path in args) {
#       var pk = EntityFormat.GetFromBytes(File.ReadAllBytes(path), EntityContext.None);
#       if (pk is null) { Console.WriteLine($"{path}\tIllegal\tformat inconnu"); continue; }
#       var la = new LegalityAnalysis(pk);
#       var why = string.Join(" | ", la.Results.Where(r => !r.Valid).Select(r => r.Identifier + " " + r.Result));
#       Console.WriteLine(la.Valid ? $"{path}\tLegal" : $"{path}\tIllegal\t{why}");
#   }
#   dotnet publish -c Release -o <dossier> ; $env:PKHEX_CHECK = "<dossier>\PkhexCheck.exe"
#
# Sans PKHEX_CHECK, le script s'arrête après l'export (code 2) : ce n'est pas un échec de
# Kaleido, seulement l'absence de l'oracle. À lancer avant chaque release.

param(
    [string]$Out = (Join-Path $env:TEMP "kaleido-pkhex-check")
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $root "target\release\kaleido.exe"

Write-Host "Compilation de la ligne de commande Kaleido (release)..."
& cargo build -p kaleido-cli --release --manifest-path (Join-Path $root "Cargo.toml")
if ($LASTEXITCODE -ne 0) { throw "cargo build a echoue" }

if (Test-Path $Out) { Remove-Item -Recurse -Force $Out -Confirm:$false }
New-Item -ItemType Directory -Force $Out | Out-Null

$data = Join-Path $root "crates\core\tests\data"
& $cli legal-dir (Join-Path $data "pkhex\legality\Legal") (Join-Path $Out "legal")
& $cli legal-dir (Join-Path $data "pkhex\legality\Illegal") (Join-Path $Out "illegal")
foreach ($gen in 4..7) {
    $corpus = Join-Path $data "smogon\gen$gen.txt"
    if (Test-Path $corpus) { & $cli legal-smogon $corpus $gen (Join-Path $Out "smogon$gen") }
}

$files = Get-ChildItem -Recurse -File $Out | Where-Object { $_.Extension -match '^\.pk[4-7]$' }
Write-Host ("{0} fichiers exportes dans {1}" -f $files.Count, $Out)

if (-not $env:PKHEX_CHECK -or -not (Test-Path $env:PKHEX_CHECK)) {
    Write-Host "PKHEX_CHECK non defini : verification PKHeX sautee (voir l'en-tete du script)."
    exit 2
}

$lines = & $env:PKHEX_CHECK ($files | ForEach-Object { $_.FullName })
$bad = @($lines | Where-Object { $_ -match "`tIllegal" })
$groups = $files | Group-Object { $_.Directory.Name }
foreach ($g in $groups) {
    $dir = $g.Name
    $n = @($bad | Where-Object { $_ -match [regex]::Escape("\$dir\") }).Count
    Write-Host ("{0,-10} {1}/{2} legaux selon PKHeX" -f $dir, ($g.Count - $n), $g.Count)
}
$bad | ForEach-Object { Write-Host $_ }
if ($bad.Count -gt 0) { exit 1 }
Write-Host "Tous les Pokemon exportes sont legaux selon PKHeX."
