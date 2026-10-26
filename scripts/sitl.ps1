# Starts one ArduPilot simulator instance in a working directory of its own.
#
# The binary writes its parameter state (`eeprom.bin`) and the terrain cells it fetches into whatever
# directory it is started from. Started from the repository, that leaves the state of one run where the
# next run reads it: a parameter an earlier test wrote, or an arming another run decided. This script
# gives each instance its own directory and clears the state before starting, which is what a fresh
# vehicle means. `-KeepState` keeps it when the state of a particular flight is the thing being looked at.
#
# It prints the address the instance listens on — the base port plus ten per instance — which is what
# `MPCRAFT_SITL_TARGET` is set to. The process stops when the station it was serving goes away, so a run
# holds one connection and walks through its areas inside it; nothing else can connect while it runs.
#
# Usage:
#   .\scripts\sitl.ps1 -Instance 0
#   .\scripts\sitl.ps1 -Vehicle ArduPlane -Instance 3 -HomeLocation "-35.36,149.17,584,353"
#   $env:MPCRAFT_SITL_TARGET = "tcp:127.0.0.1:5760"
#   cargo +stable-x86_64-pc-windows-gnu test -p mpcraft-link --features sitl-tests --test sitl -- --test-threads=1

param(
    [int]$Instance = 0,
    [ValidateSet('ArduCopter', 'ArduPlane', 'ArduRover')][string]$Vehicle = 'ArduCopter',
    # Not `-Home`: PowerShell reserves `$Home` as the user's home directory and will not be assigned to.
    [string]$HomeLocation = '-35.363261,149.165364,584,353',
    [string]$Model = '+',
    [int]$Speedup = 1,
    [switch]$KeepState,
    [string]$SitlDir = "$env:MPCRAFT_SITL_DIR",
    [string]$WorkingDir
)

$ErrorActionPreference = 'Stop'

if (-not $SitlDir) {
    $SitlDir = 'C:\Users\Byte.DESKTOP-JN9IU2Q\Documents\Mission Planner\sitl'
}

$binary = Join-Path $SitlDir "$Vehicle.exe"
if (-not (Test-Path $binary)) {
    Write-Error "No simulator binary at $binary. The parameter state and the defaults file belong to that directory, so set -SitlDir or MPCRAFT_SITL_DIR where the binaries are."
    exit 1
}

# The defaults file is named for the vehicle rather than for the binary, which is what the binary reads
# when it is started without one.
$paramFile = Join-Path $SitlDir "default_params\$($Vehicle -replace 'Ardu', '').parm"
if (-not (Test-Path $paramFile)) {
    Write-Error "No defaults file at $paramFile. A run without one starts with whatever the previous run left in the state."
    exit 1
}

if (-not $WorkingDir) {
    # One directory per instance: two instances on two ports must not share one state.
    $WorkingDir = Join-Path $env:TEMP "mpcraft-sitl\instance-$Instance"
}

New-Item -ItemType Directory -Path $WorkingDir -Force | Out-Null

$state = Join-Path $WorkingDir 'eeprom.bin'
if (Test-Path $state) {
    if ($KeepState) {
        Write-Host "State kept: $state"
    }
    else {
        Remove-Item $state
        Write-Host 'State cleared: a fresh instance does not inherit what an earlier run wrote.'
    }
}

$port = 5760 + 10 * $Instance

Write-Host "Working directory: $WorkingDir"
Write-Host "Listening on tcp:127.0.0.1:$port"

Push-Location $WorkingDir
try {
    & $binary -M $Model -s $Speedup --serial0 'tcp:0' --defaults $paramFile --instance $Instance --home $HomeLocation
}
finally {
    Pop-Location
}
