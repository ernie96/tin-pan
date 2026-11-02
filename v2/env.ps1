# Sets up this PowerShell session for building Tin Pan 2 on Windows: the Visual Studio C++ tools (MSVC),
# the CMake that comes with them, and libclang for bindgen. wxDragon builds wxWidgets from source with these
# the first time, which takes a while; later builds reuse it.
#
#   . .\env.ps1        (note the dot, so the settings stay in this session)
#   cargo build --release

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path $vswhere)) { throw 'Visual Studio Build Tools with C++ are needed (vswhere.exe not found).' }
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'Visual Studio has no C++ tools installed (the "Desktop development with C++" workload).' }

$vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'
cmd /c "`"$vcvars`" >nul && set" | ForEach-Object {
    if ($_ -match '^([^=]+)=(.*)$') { Set-Item -Path "env:$($Matches[1])" -Value $Matches[2] }
}

# Visual Studio's own CMake and Ninja go first: a ninja "shim" from another tool (pyenv, say) can fail to run.
foreach ($dir in 'Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja', 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin') {
    $path = Join-Path $vs $dir
    if (Test-Path $path) { $env:PATH = "$path;$env:PATH" }
}

if (-not $env:LIBCLANG_PATH) {
    foreach ($dir in @("$env:ProgramFiles\LLVM\bin", (Join-Path $vs 'VC\Tools\Llvm\x64\bin'))) {
        if (Test-Path (Join-Path $dir 'libclang.dll')) { $env:LIBCLANG_PATH = $dir; break }
    }
}

foreach ($tool in 'cl', 'cmake', 'ninja') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Write-Warning "$tool isn't on the PATH." }
}
if (-not $env:LIBCLANG_PATH) { Write-Warning 'libclang.dll not found; install LLVM or set LIBCLANG_PATH.' }
