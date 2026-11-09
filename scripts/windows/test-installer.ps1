param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$Version,
    [ValidateSet('x64', 'arm64')][string]$Architecture = 'x64'
)
$ErrorActionPreference = 'Stop'
$Installer = (Resolve-Path $Installer).Path
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Datalith'
if (Test-Path $key) { throw 'Use a disposable Windows runner without Datalith installed.' }
$directory = Join-Path $env:LOCALAPPDATA 'Datalith'
if (Test-Path $directory) { throw "Existing directory: $directory" }
function Invoke-Installer {
    $process = Start-Process -FilePath $Installer -ArgumentList '/S' -PassThru
    if (-not $process.WaitForExit(120000)) { $process.Kill(); throw 'Installation timed out' }
    if ($process.ExitCode -ne 0) { throw "Installer exit code: $($process.ExitCode)" }
    $entry = Get-ItemProperty $key
    if ($entry.DisplayVersion -cne $Version -or $entry.Publisher -cne 'Mycelium Build') {
        throw 'Unexpected Add/Remove Programs metadata'
    }
    $binary = Join-Path $directory 'datalith.exe'
    $bytes = [IO.File]::ReadAllBytes($binary)
    $offset = [BitConverter]::ToInt32($bytes, 0x3c)
    $machine = [BitConverter]::ToUInt16($bytes, $offset + 4)
    $expected = if ($Architecture -eq 'arm64') { 0xaa64 } else { 0x8664 }
    if ($machine -ne $expected) { throw 'Unexpected installed PE architecture' }
    $identity = & $binary --print-build-identity | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $identity.version -cne $Version -or $identity.channel -cne 'stable') {
        throw 'Unexpected installed build identity'
    }
}
Invoke-Installer
Invoke-Installer
$uninstaller = Join-Path $directory 'uninstall.exe'
$process = Start-Process -FilePath $uninstaller -ArgumentList '/S' -PassThru
if (-not $process.WaitForExit(120000)) { $process.Kill(); throw 'Uninstallation timed out' }
if ($process.ExitCode -ne 0) { throw "Uninstaller exit code: $($process.ExitCode)" }
$deadline = [DateTime]::UtcNow.AddSeconds(60)
while ((Test-Path $key) -or (Test-Path (Join-Path $directory 'datalith.exe'))) {
    if ([DateTime]::UtcNow -gt $deadline) { throw 'Uninstall left registration or executable behind' }
    Start-Sleep -Milliseconds 500
}
Write-Output "Validated silent install, reinstall and uninstall: $Version $Architecture"
