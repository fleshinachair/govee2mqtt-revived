# Run cargo on native Windows using MSYS2 MinGW-w64 and the
# x86_64-pc-windows-gnu Rust toolchain. Linux and CI do not need this.
#
#   powershell -ExecutionPolicy Bypass -File scripts\win-gnu-cargo.ps1 test
#
# libmosquitto-sys links `libssl` and `libcrypto` on every Windows target (the
# MSVC names). On GNU, rustc therefore looks for liblibssl.a, but the vendored
# OpenSSL build produces libssl.a. This script adds the aliases next to them.

# usr\bin goes last: it has a `cmd` that shadows cmd.exe.
$env:PATH = @(
    'C:\msys64\mingw64\bin'
    [Environment]::GetEnvironmentVariable('Path', 'Machine')
    [Environment]::GetEnvironmentVariable('Path', 'User')
    'C:\msys64\usr\bin'
) -join ';'

# Link with MinGW gcc even if ~/.cargo/config.toml names another linker (a zig
# wrapper, for example, cannot resolve the winapi_* import libraries).
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = 'C:\msys64\mingw64\bin\gcc.exe'

$root = Split-Path -Parent $PSScriptRoot

function Add-OpenSslAlias {
    $added = 0
    $target = Join-Path $root 'target'
    if (-not (Test-Path $target)) { return 0 }
    Get-ChildItem -Path $target -Recurse -File -Include 'libssl.a', 'libcrypto.a' |
        Where-Object { $_.DirectoryName -like '*openssl-build\install\lib*' } |
        ForEach-Object {
            $alias = Join-Path $_.DirectoryName ('lib' + $_.Name)
            if (-not (Test-Path $alias)) {
                Copy-Item $_.FullName $alias
                $added++
            }
        }
    return $added
}

Push-Location $root
try {
    $null = Add-OpenSslAlias
    & cargo @args
    $code = $LASTEXITCODE
    # On a clean tree OpenSSL is built during this run, so the first attempt
    # fails at libmosquitto-sys. Add the aliases and try once more.
    if ($code -ne 0 -and (Add-OpenSslAlias) -gt 0) {
        Write-Host 'Added OpenSSL library aliases; retrying.'
        & cargo @args
        $code = $LASTEXITCODE
    }
} finally {
    Pop-Location
}
exit $code
