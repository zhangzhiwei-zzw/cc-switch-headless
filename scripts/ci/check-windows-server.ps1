param(
    [Parameter(Mandatory = $true)]
    [string]$Binary
)

$ErrorActionPreference = "Stop"
$binaryPath = (Resolve-Path $Binary).Path
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ServerResources {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr LoadLibraryExW(string path, IntPtr file, uint flags);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr FindResourceW(IntPtr module, IntPtr name, IntPtr type);
    [DllImport("kernel32.dll")]
    public static extern bool FreeLibrary(IntPtr module);
}
'@

# Read resources as data: do not execute the server or load its dependencies.
$module = [ServerResources]::LoadLibraryExW($binaryPath, [IntPtr]::Zero, 2)
if ($module -eq [IntPtr]::Zero) { throw "Cannot load executable resources: $binaryPath" }
try {
    foreach ($resource in @(
        @{ Name = 32512; Type = 14; Label = "application icon group" },
        @{ Name = 1; Type = 24; Label = "application manifest" }
    )) {
        $handle = [ServerResources]::FindResourceW($module, [IntPtr]$resource.Name, [IntPtr]$resource.Type)
        if ($handle -eq [IntPtr]::Zero) { throw "Missing $($resource.Label) in $binaryPath" }
    }
    $description = (Get-Item $binaryPath).VersionInfo.FileDescription
    if ($description -ne "CC Switch headless Web server") { throw "Unexpected file description: $description" }
    Write-Host "Windows server icon, manifest and version resources verified."
} finally {
    [void][ServerResources]::FreeLibrary($module)
}
