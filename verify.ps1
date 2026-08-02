<#
.SYNOPSIS
    Every gate this game has, in one command.

.DESCRIPTION
    Forty-two systems have each added a check, and every one of them found a
    real defect the first time it ran. None of that is worth anything if running
    them depends on remembering they exist — a gate that is not run is a gate
    that does not exist, and this session has already skipped the million-spin
    RTP run except when something looked wrong.

    So this is the list, made executable. It fails on the first hard failure and
    reports what passed, because a summary of nine greens and a red is easier to
    act on than a wall of output.

    The audits need a real window and a real font (§5.37), so they run through
    the capture harness rather than as tests. That is why this is a script and
    not another `cargo test`.

.PARAMETER Long
    Also run the million-spin RTP gate and the 200,000-round conservation soak.
    Minutes rather than seconds; the right thing before a release and the wrong
    thing on every edit.

.PARAMETER SkipBuild
    Use the release binary already in the target directory. For iterating on the
    audit matrix itself.
#>
[CmdletBinding()]
param(
    [switch]$Long,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$ProjectRoot = $PSScriptRoot
$ToolkitRoot = Join-Path (Split-Path $ProjectRoot -Parent) 'macroquad-toolkit'
$Exe = Join-Path (Split-Path (Split-Path $ProjectRoot -Parent) -Parent) '.cargo-target\release\dragons_hoard.exe'

$script:Results = @()
$script:Failed = $false

function Step {
    param([string]$Name, [scriptblock]$Body)

    if ($script:Failed) { return }
    Write-Host "  $Name ... " -NoNewline
    $started = Get-Date
    try {
        & $Body
        $took = [int]((Get-Date) - $started).TotalMilliseconds
        Write-Host "ok" -ForegroundColor Green -NoNewline
        Write-Host " (${took}ms)" -ForegroundColor DarkGray
        $script:Results += [pscustomobject]@{ Name = $Name; Ok = $true }
    } catch {
        Write-Host "FAILED" -ForegroundColor Red
        Write-Host $_.Exception.Message -ForegroundColor Red
        $script:Results += [pscustomobject]@{ Name = $Name; Ok = $false }
        $script:Failed = $true
    }
}

function Cargo {
    param([string]$Dir, [string]$What, [string[]]$Argv)
    Push-Location $Dir
    # Cargo writes progress to stderr, and with ErrorActionPreference at Stop
    # PowerShell turns that into a thrown exception before the exit code is ever
    # read — so a clean clippy run reported as a failure.
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = # cargo.exe, not cargo: this function is named Cargo, and PowerShell resolves
        # a function before an executable — so the unqualified name called *this*,
        # which is why the first run overflowed the call stack.
        & cargo.exe @Argv 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw ($What + "`n" + ($output | Select-Object -Last 30 | Out-String))
        }
    } finally {
        $ErrorActionPreference = $previous
        Pop-Location
    }
}

# The audits print their findings and exit non-zero when something is wrong, so
# the exit code is the verdict and the output is the explanation.
function Audit {
    param([string]$Scene, [hashtable]$Vars, [string]$What)

    # Start from a known state. A run that inherits the previous step's knobs
    # measures a screen nobody asked about, and the findings look like faults on
    # whatever screen happened to be next (§5.50).
    foreach ($key in 'DRAGONS_HOARD_THEME', 'DRAGONS_HOARD_TEXT_SCALE', 'DRAGONS_HOARD_PSEUDO',
                     'DRAGONS_HOARD_WINDOW_WIDTH', 'DRAGONS_HOARD_WINDOW_HEIGHT',
                     'DRAGONS_HOARD_CAPTURE_STRIP', 'DRAGONS_HOARD_CAPTURE_STRIP_EVERY') {
        if (-not ($Vars -and $Vars.ContainsKey($key))) {
            [Environment]::SetEnvironmentVariable($key, $null)
        }
    }

    $saved = @{}
    foreach ($key in $Vars.Keys) {
        $saved[$key] = [Environment]::GetEnvironmentVariable($key)
        [Environment]::SetEnvironmentVariable($key, $Vars[$key])
    }
    [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', $Scene)
    # `audit:<screen>` carries a colon, which Windows will not take in a path.
    $file = 'verify_' + ($Scene -replace '[^A-Za-z0-9_]', '_') + '.png'
    [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_PATH', (Join-Path $env:TEMP $file))
    if (-not ($Vars -and $Vars.ContainsKey('DRAGONS_HOARD_CAPTURE_FRAMES'))) {
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_FRAMES', '3')
    }
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & $Exe 2>&1
        $findings = $output | Where-Object { $_ -match 'audit: \d+ findings|overlap by' }
        if ($LASTEXITCODE -ne 0 -or $findings) {
            throw "$What`n$($output | Out-String)"
        }
    } finally {
        $ErrorActionPreference = $previous
        foreach ($key in $Vars.Keys) {
            [Environment]::SetEnvironmentVariable($key, $saved[$key])
        }
        foreach ($key in 'DRAGONS_HOARD_CAPTURE_SCENE', 'DRAGONS_HOARD_CAPTURE_PATH', 'DRAGONS_HOARD_CAPTURE_FRAMES') {
            [Environment]::SetEnvironmentVariable($key, $null)
        }
    }
}

Write-Host ''
Write-Host "Dragon's Hoard - verification" -ForegroundColor Cyan
Write-Host ''

# The 800-line file gate born here (§5.54, blank-line fix §5.69) now lives in
# `macroquad_toolkit::source_gate`, run by every game's `tests/code_standards.rs`
# — so the toolkit-tests and game-tests steps below enforce it, per the current
# CODE_STANDARDS accounting (non-test lines only).

Write-Host 'Source' -ForegroundColor Cyan
Step 'toolkit format' { Cargo -Dir $ToolkitRoot -What 'toolkit is unformatted' -Argv @('fmt','--','--check') }
Step 'toolkit lint'   { Cargo -Dir $ToolkitRoot -What 'toolkit lints' -Argv @('clippy','--all-targets','--','-D','warnings') }
Step 'toolkit tests'  { Cargo -Dir $ToolkitRoot -What 'toolkit tests failed' -Argv @('test','--quiet') }
Step 'game format'    { Cargo -Dir $ProjectRoot -What 'game is unformatted' -Argv @('fmt','--','--check') }
Step 'game lint'      { Cargo -Dir $ProjectRoot -What 'game lints' -Argv @('clippy','--all-targets','--all-features','--','-D','warnings') }
Step 'game tests'     { Cargo -Dir $ProjectRoot -What 'game tests failed' -Argv @('test','--quiet') }
Step 'wasm build'     { Cargo -Dir $ProjectRoot -What 'wasm build failed' -Argv @('build','--release','--target','wasm32-unknown-unknown') }

if (-not $SkipBuild) {
    Step 'release build' { Cargo -Dir $ProjectRoot -What 'release build failed' -Argv @('build','--release') }
}
if (-not (Test-Path $Exe)) {
    Write-Host "  no release binary at $Exe" -ForegroundColor Red
    exit 1
}

# Each axis is varied on its own rather than crossed with the others. The full
# product is 108 runs of a thing that has never failed on two axes at once, and
# a gate nobody waits for is a gate nobody runs.
Write-Host ''
Write-Host 'Layout' -ForegroundColor Cyan
Step 'themes'     { foreach ($t in 'hoard', 'frost', 'ember', 'spire', 'slide', 'tidepool') { Audit -Scene 'layout_audit' -Vars @{ DRAGONS_HOARD_THEME = $t } -What "layout breaks under the $t theme" } }
Step 'text sizes' { foreach ($s in '1.0', '1.15', '1.3') { Audit -Scene 'layout_audit' -Vars @{ DRAGONS_HOARD_TEXT_SCALE = $s } -What "layout breaks at $s text" } }
Step 'pseudolocale' { Audit -Scene 'layout_audit' -Vars @{ DRAGONS_HOARD_PSEUDO = '1' } -What 'layout will not take a 40% translation' }
Step 'aspects'    { foreach ($w in '1000', '1280', '1600') { Audit -Scene 'layout_audit' -Vars @{ DRAGONS_HOARD_WINDOW_WIDTH = $w; DRAGONS_HOARD_WINDOW_HEIGHT = '720' } -What "layout breaks at ${w}x720" } }

Write-Host ''
Write-Host 'The published build' -ForegroundColor Cyan
# Everything about the web build had been verified by reasoning and a native
# binary. The wasm's import section is a precise contract — every `env.<name>` in
# it must be provided by a script the page loads — and nothing had ever read it.
# It found six GL entry points missing, because the runtime was downloaded from a
# samples website rather than taken from the crate the games compile against, and
# miniquad stubs anything absent instead of failing (§5.62).
Step 'the page provides what the wasm asks for' {
    $root = if ($env:PREVIEW_GAMES_ROOT) { $env:PREVIEW_GAMES_ROOT } else { 'D:/xampp/htdocs/games' }
    $deployed = Join-Path $root 'dragons_hoard'
    if (-not (Test-Path (Join-Path $deployed 'index.html'))) {
        Write-Host '(not published yet) ' -NoNewline -ForegroundColor DarkGray
        return
    }
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $checker = Join-Path $ProjectRoot 'tools/check_web_build.py'
        $output = & python $checker $deployed 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw ("the published web build is missing something the wasm imports`n" + ($output | Out-String))
        }
    } finally {
        $ErrorActionPreference = $previous
    }
}

Write-Host ''
Write-Host "The player's game" -ForegroundColor Cyan
# The harness used to overwrite the save it was verifying (§5.55). Every capture
# scene deals a fabricated state, the loop autosaves when a spin resolves, and
# this script runs about fifty of them — so running the verification suite
# destroyed the player's game, every time, and nothing said so.
# Boot has to read what the autosave wrote (§5.58). It never did: Game::new
# built a fresh session and never looked at the disk, so every launch reset the
# hoard meter and three of the four progressives, and nothing noticed — writing a
# save nobody reads looks exactly like writing one that is read.
Step 'the game remembers' {
    $dir = Join-Path $env:LOCALAPPDATA 'dragons_hoard'
    $slot = Join-Path $dir 'save_dragon_autosave.json'
    if (-not (Test-Path $slot)) { return }
    $before = Get-Content $slot -Raw
    try {
        $save = $before | ConvertFrom-Json
        $save.data.hoard.count = 11
        $save.data.hoard.pot = 2468
        ($save | ConvertTo-Json -Depth 12) | Set-Content $slot -NoNewline
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_PATH', (Join-Path $env:TEMP 'verify_boot.png'))
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', 'boot_report')
        $report = & $Exe 2>&1 | Where-Object { $_ -match '^boot ' }
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_PATH', $null)
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', $null)
        if ($report -notmatch 'eggs 11' -or $report -notmatch 'pot 2468') {
            throw "the game started without reading its own save`n$report"
        }
    } finally {
        Set-Content $slot $before -NoNewline
    }
}

Step 'saves are left alone' {
    $dir = Join-Path $env:LOCALAPPDATA 'dragons_hoard'
    if (-not (Test-Path $dir)) { return }
    $before = Get-ChildItem $dir -Filter *.json | ForEach-Object { "$($_.Name):$((Get-FileHash $_.FullName).Hash)" }
    foreach ($scene in 'win', 'hatch', 'jackpot', 'ruin', 'wallet_walk') {
        Audit -Scene $scene -Vars @{ DRAGONS_HOARD_CAPTURE_FRAMES = '150' } -What "the $scene scene failed"
    }
    $after = Get-ChildItem $dir -Filter *.json | ForEach-Object { "$($_.Name):$((Get-FileHash $_.FullName).Hash)" }
    $changed = Compare-Object $before $after
    if ($changed) {
        throw ("capture scenes wrote to the player's saves`n" + ($changed | Out-String))
    }
}

Write-Host ''
Write-Host 'One screen at a time' -ForegroundColor Cyan
# Every screen in the registry, measured the same way (§5.50). The list is not
# repeated here on purpose — `audit:<id>` looks the screen up in `Screen::ALL`
# and asserts it actually opened, so a screen added to the game and forgotten
# here fails loudly rather than going unmeasured for twelve iterations.
# Asked for rather than repeated. This list used to live here, and by the time
# the ruin screen (§5.53) was registered, tested and reachable, the sweep still
# did not know it existed — which is the exact drift §5.50 built the registry to
# stop, reintroduced one file over.
[Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_PATH', (Join-Path $env:TEMP 'verify_screens.png'))
[Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', 'screens')
$Screens = @(& $Exe 2>&1 | Where-Object { $_ -match '^screen ' } | ForEach-Object { ($_ -split ' ')[1] })
[Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_PATH', $null)
[Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', $null)
if ($Screens.Count -lt 10) { throw "the game listed $($Screens.Count) screens; the registry is not being read" }
Step 'every screen' {
    foreach ($screen in $Screens) {
        Audit -Scene "audit:$screen" -What "the $screen screen has a layout, contrast or collision fault"
    }
}
# The one crossed pair in this harness, and it earned the exception: sweeping
# the screens under a 40% translation found an overlay slicing the cabinet name
# that was invisible at English widths (§5.50).
Step 'every screen, translated' {
    foreach ($screen in $Screens) {
        Audit -Scene "audit:$screen" -Vars @{ DRAGONS_HOARD_PSEUDO = '1' } -What "the $screen screen breaks under a 40% translation"
    }
}
# Motion (§5.52). A settled frame cannot show a fault that only exists while
# something is moving, and both bugs a player reported were of exactly that kind.
# 300 frames is past the longest spin in the catalog on purpose: a run that ends
# before the reels stop never evaluates the invariant and reports clean, which is
# what the first version of this did.
Step 'motion' {
    foreach ($cabinet in 'dragon', 'frost', 'ways', 'wyrmspire', 'avalanche', 'tidepool') {
        Audit -Scene "motion:$cabinet" -Vars @{
            DRAGONS_HOARD_CAPTURE_FRAMES = '300'
            DRAGONS_HOARD_CAPTURE_STRIP = '1'
            DRAGONS_HOARD_CAPTURE_STRIP_EVERY = '20'
        } -What "the $cabinet reels do something wrong while they are turning"
    }
}
# The number, not the silence.
#
# This gate ran for eight sections and never measured anything: every button
# declared a region over itself for the contrast check, the region occluded the
# control it was the face of, and so the neighbour list was empty at the end of
# every frame and every report was suppressed behind `neighbours_warm`. It
# passed by saying nothing (§5.77).
#
# So it now reads the figure and compares it. 1080 CSS pixels is an iPad in
# landscape with the canvas filling the screen, which is the narrowest device
# this game claims to be playable on.
Step 'a tablet can actually press it' {
    # 960 is the game's own logical width at 4:3, and the floor: a control
    # cannot clear 44 CSS pixels on a canvas narrower than the layout it is
    # drawn in unless it exceeds 44 logical pixels. §5.78 got the requirement
    # down from 982 to exactly this, so the gate now holds it there rather than
    # at the tablet figure it used to allow.
    $needed = 960
    foreach ($scene in 'touch_audit', 'touch_audit_settings', 'touch_audit_buy') {
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_WINDOW_WIDTH', '1080')
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_WINDOW_HEIGHT', '810')
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', $scene)
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_PATH', (Join-Path $env:TEMP 'verify_touch.png'))
        $out = & $Exe 2>&1
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_WINDOW_WIDTH', $null)
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_WINDOW_HEIGHT', $null)
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', $null)
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_PATH', $null)

        $lines = @($out | Where-Object { $_ -match 'need a (\d+)px-wide window' })
        if (-not $lines) {
            throw "$scene reported no touch-target measurement at all, which is how this gate passed for eight sections"
        }
        $worst = 0
        $who = ''
        foreach ($line in $lines) {
            if ($line -match 'need a (\d+)px-wide window; worst is (.+)$') {
                if ([int]$matches[1] -gt $worst) { $worst = [int]$matches[1]; $who = $matches[2] }
            }
        }
        if ($worst -gt $needed) {
            throw "$scene needs a ${worst}px canvas for every control to clear 44 CSS pixels; the floor is $needed. Worst control: $who"
        }
    }

    # And the other half: drawn size, not hit size (§5.78).
    #
    # A control's hit area is grown to the standard, so a 26px button is
    # reachable. It is still a 26px button to look at and aim for, and every
    # button in this game was between 26 and 38 until the audit was finally
    # able to say so. This sweeps the whole registry, because the target audit
    # used to be armed in three hand-written scenes while the layout audit swept
    # all twenty-one.
    [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_PATH', (Join-Path $env:TEMP 'verify_screens.png'))
    [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', 'screens')
    $all = @(& $Exe 2>&1 | Where-Object { $_ -match '^screen ' } | ForEach-Object { ($_ -split ' ')[1] })
    foreach ($screen in $all) {
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', "audit:$screen")
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_WINDOW_WIDTH', '1080')
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_WINDOW_HEIGHT', '810')
        $small = @(& $Exe 2>&1 | Where-Object { $_ -match 'drawn (\d+)px' } | Select-Object -Unique)
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_WINDOW_WIDTH', $null)
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_WINDOW_HEIGHT', $null)
        if ($small) {
            throw ("the $screen screen draws controls under 44 logical pixels`n  " + ($small -join "`n  "))
        }
    }
    [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_SCENE', $null)
    [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_CAPTURE_PATH', $null)
}
Step 'collisions and touch targets' {
    foreach ($scene in 'touch_audit', 'touch_audit_settings', 'touch_audit_buy') {
        foreach ($w in '1000', '1280') {
            Audit -Scene $scene -Vars @{ DRAGONS_HOARD_WINDOW_WIDTH = $w; DRAGONS_HOARD_WINDOW_HEIGHT = '720' } -What "$scene collides at ${w}x720"
        }
    }
}

Write-Host ''
Write-Host 'A player at random' -ForegroundColor Cyan
Step 'ten thousand presses' {
    # Three seeds rather than one. The harness is deterministic on purpose, so a
    # single seed is a single evening — and the two faults it found while being
    # written both showed on some seeds and not others (§5.76).
    foreach ($seed in '946309677721361986', '11', '22') {
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_DRIFT', '10000')
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_DRIFT_SEED', $seed)
        $out = & $Exe 2>&1 | Where-Object { $_ -notmatch '^warn' }
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_DRIFT', $null)
        [Environment]::SetEnvironmentVariable('DRAGONS_HOARD_DRIFT_SEED', $null)
        if ($LASTEXITCODE -ne 0) {
            throw ("the game broke under random play (seed $seed)`n  " + ($out -join "`n  "))
        }
    }
}

if ($Long) {
    Write-Host ''
    Write-Host 'Maths' -ForegroundColor Cyan
    Step 'million-spin RTP' { Cargo -Dir $ProjectRoot -What 'a cabinet is outside its RTP band' -Argv @('test','--release','every_machine_holds_its_rtp','--','--ignored','--quiet') }
    Step 'conservation soak' { Cargo -Dir $ProjectRoot -What 'the interactive path does not reconcile' -Argv @('test','--release','soak','--','--ignored','--quiet') }
}

Write-Host ''
if ($script:Failed) {
    Write-Host 'FAILED' -ForegroundColor Red
    exit 1
}
Write-Host "$($script:Results.Count) gates passed" -ForegroundColor Green
if (-not $Long) {
    Write-Host 'RTP and soak not run; use -Long before shipping.' -ForegroundColor DarkGray
}
Write-Host ''
