<#
.SYNOPSIS
    Downloads the pinned ffmpeg build that LeagueRecord bundles for the Auto-Clip feature.

.DESCRIPTION
    Auto-Clip cuts highlight clips out of a recording with a stream copy, which needs an
    ffmpeg binary. The binary is not checked into git (85 MB), so it is fetched here and
    dropped at src-tauri/bin/ffmpeg.exe, from where tauri.conf.json bundles it as a resource.

    The download is a pinned, immutable GitHub release asset and is verified against a
    SHA256 hash recorded in this script. A mismatch aborts without writing anything.

    build.rs calls this automatically when src-tauri/bin/ffmpeg.exe is missing, so a plain
    `bun run tauri build` still works from a clean checkout.

.PARAMETER Force
    Re-download even if src-tauri/bin/ffmpeg.exe already exists.
#>

[CmdletBinding()]
param(
    [switch] $Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# pinned release - update Url and Sha256 together, never one without the other
$Url = 'https://github.com/GyanD/codexffmpeg/releases/download/7.1/ffmpeg-7.1-essentials_build.zip'
$Sha256 = 'FA7D4D7E795DB0E2503F49F105F46ED5852386F0CFDD819899BE3B65EBDE24FC'

$repoRoot = Split-Path -Parent $PSScriptRoot
$targetDir = Join-Path $repoRoot 'src-tauri/bin'
$target = Join-Path $targetDir 'ffmpeg.exe'

if ((Test-Path -LiteralPath $target) -and -not $Force) {
    Write-Information "ffmpeg already present at $target - nothing to do (use -Force to re-download)." -InformationAction Continue
    exit 0
}

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("leaguerecord-ffmpeg-" + [System.Guid]::NewGuid().ToString('n'))
$zip = Join-Path $work 'ffmpeg.zip'

try {
    New-Item -ItemType Directory -Path $work -Force | Out-Null
    New-Item -ItemType Directory -Path $targetDir -Force | Out-Null

    Write-Information "Downloading $Url" -InformationAction Continue
    $previousProgress = $ProgressPreference
    $ProgressPreference = 'SilentlyContinue'   # the progress bar makes Invoke-WebRequest an order of magnitude slower
    try {
        Invoke-WebRequest -Uri $Url -OutFile $zip -MaximumRedirection 5 -ErrorAction Stop
    } finally {
        $ProgressPreference = $previousProgress
    }

    $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256 -ErrorAction Stop).Hash
    if ($actual -ne $Sha256) {
        throw "SHA256 mismatch for the ffmpeg download.`n  expected: $Sha256`n  actual:   $actual`nRefusing to install an unverified binary."
    }
    Write-Information "SHA256 verified." -InformationAction Continue

    $extract = Join-Path $work 'extract'
    Expand-Archive -LiteralPath $zip -DestinationPath $extract -Force -ErrorAction Stop

    $exe = Get-ChildItem -LiteralPath $extract -Recurse -Filter 'ffmpeg.exe' -ErrorAction Stop | Select-Object -First 1
    if ($null -eq $exe) {
        throw "no ffmpeg.exe inside the downloaded archive"
    }

    Copy-Item -LiteralPath $exe.FullName -Destination $target -Force -ErrorAction Stop
    $sizeMb = [math]::Round((Get-Item -LiteralPath $target).Length / 1MB, 1)
    Write-Information "Installed ffmpeg.exe ($sizeMb MB) to $target" -InformationAction Continue
} catch {
    Write-Error "failed to fetch ffmpeg: $($_.Exception.Message)"
    exit 1
} finally {
    if (Test-Path -LiteralPath $work) {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
}
