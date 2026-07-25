<#
.SYNOPSIS
    Headless screenshot harness for Dragon's Hoard.

.DESCRIPTION
    Thin wrapper around the shared macroquad-toolkit capture script. Builds the
    debug exe and drives it through the env-var capture hook
    (DRAGONS_HOARD_CAPTURE_*) provided by macroquad_toolkit::capture in
    src/main.rs. The game boots straight into the idle reel screen, so the
    scene name currently only picks the output filename.

.EXAMPLE
    ./scripts/capture_ui.ps1
    ./scripts/capture_ui.ps1 -Scenes idle -Frames 60 -SkipBuild
#>
param(
    [string[]]$Scenes = @("idle"),
    [int]$Frames = 150,
    [string]$OutputDir = "docs\verification",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$gameDir = Split-Path -Parent $PSScriptRoot
$shared = Join-Path (Split-Path -Parent $gameDir) "macroquad-toolkit\scripts\capture_ui.ps1"

& $shared -GameDir $gameDir -Prefix "DRAGONS_HOARD" -Scenes $Scenes -Frames $Frames -OutputDir $OutputDir -SkipBuild:$SkipBuild
