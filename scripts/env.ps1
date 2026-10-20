# MPCraft developer environment (Windows / PowerShell)
#
# The workspace normally builds with the MSVC toolchain. This script selects a working
# toolchain and prepares the PATH, which is what CI and local builds expect.
#
# Usage:
#   . .\scripts\env.ps1
#   cargo test --workspace

$ErrorActionPreference = 'Stop'

# Prefer the MSVC toolchain. Fall back to the GNU toolchain when the Visual Studio build
# tools (or the Windows SDK) are not installed, since GNU ships its own linker.
$default = (& rustup show active-toolchain 2>$null) -split ' ' | Select-Object -First 1

if ($default -notmatch 'gnu' -and -not (Test-Path "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe")) {
    Write-Warning 'No Visual Studio build tools found; using the GNU toolchain.'
    & rustup default stable-x86_64-pc-windows-gnu
} elseif ($default -notmatch 'gnu') {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $msvc = & $vswhere -products * -format value -property installationPath
    if (-not $msvc) {
        Write-Warning 'No Visual Studio build tools found; using the GNU toolchain.'
        & rustup default stable-x86_64-pc-windows-gnu
    }
}

# The GNU toolchain needs a working `make` for crates that build native code. The MinGW
# toolchain ships one; the AVR toolchain that ships with some Arduino installations also
# ships a `make.exe` that cannot build native code, so it is removed from the PATH.
$env:PATH = ($env:PATH -split ';' | Where-Object { $_ -notlike '*WinAVR*' }) -join ';'

$mingw = 'C:\Users\Byte.DESKTOP-JN9IU2Q\winlibs\mingw64\bin'
if (Test-Path $mingw) {
    $env:PATH = "$mingw;$env:PATH"
}

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

Write-Host "MPCraft environment ready."
Write-Host "  rustc: $(& rustc --version)"
