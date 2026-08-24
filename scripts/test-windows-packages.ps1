param(
  [Parameter(Mandatory = $true)]
  [string]$ArtifactDirectory
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-SinglePackage {
  param(
    [string]$Pattern,
    [string]$Description
  )

  $packages = @(Get-ChildItem -LiteralPath $script:ArtifactRoot -File -Filter $Pattern)
  if ($packages.Count -ne 1) {
    throw "Expected one $Description package matching '$Pattern'; found $($packages.Count)"
  }
  return $packages[0]
}

function Assert-Checksums {
  $manifest = Join-Path $script:ArtifactRoot "SHA256SUMS-windows.txt"
  if (!(Test-Path -LiteralPath $manifest -PathType Leaf)) {
    throw "Missing checksum manifest: $manifest"
  }

  foreach ($line in Get-Content -LiteralPath $manifest) {
    if ($line -notmatch '^([0-9a-f]{64})  (.+)$') {
      throw "Invalid checksum line: $line"
    }
    $expected = $Matches[1]
    $fileName = $Matches[2]
    $packagePath = Join-Path $script:ArtifactRoot $fileName
    $actual = (Get-FileHash -LiteralPath $packagePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
      throw "Checksum mismatch for $fileName"
    }
  }
}

function Invoke-CheckedProcess {
  param(
    [string]$FilePath,
    [string]$ArgumentList
  )

  $process = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -Wait -PassThru
  if ($process.ExitCode -ne 0) {
    throw "Process '$FilePath' exited with code $($process.ExitCode)"
  }
}

function Test-ApplicationStartup {
  param(
    [string]$ApplicationPath,
    [string]$PackageDescription
  )

  $env:CQA_NO_AI = "1"
  $application = Start-Process -FilePath $ApplicationPath -WindowStyle Hidden -PassThru
  try {
    Start-Sleep -Seconds 5
    $application.Refresh()
    if ($application.HasExited) {
      throw "$PackageDescription application exited during startup with code $($application.ExitCode)"
    }
  } finally {
    $application.Refresh()
    if (!$application.HasExited) {
      Stop-Process -Id $application.Id -Force
    }
  }
}

$script:ArtifactRoot = (Resolve-Path -LiteralPath $ArtifactDirectory).Path
$msi = Assert-SinglePackage -Pattern "*.msi" -Description "MSI"
$nsis = Assert-SinglePackage -Pattern "*-setup.exe" -Description "NSIS"
Assert-Checksums

$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) "cqa-package-test-$PID"
$msiRoot = Join-Path $testRoot "msi"
$nsisRoot = Join-Path $testRoot "nsis"
New-Item -ItemType Directory -Force -Path $msiRoot, $nsisRoot | Out-Null

try {
  $msiArguments = "/a `"$($msi.FullName)`" /qn TARGETDIR=`"$msiRoot`""
  Invoke-CheckedProcess -FilePath "msiexec.exe" -ArgumentList $msiArguments
  $msiApplication = Get-ChildItem -LiteralPath $msiRoot -Recurse -File -Filter "code-quest-advance.exe" | Select-Object -First 1
  if ($null -eq $msiApplication) {
    throw "MSI administrative extraction did not contain code-quest-advance.exe"
  }
  Test-ApplicationStartup -ApplicationPath $msiApplication.FullName -PackageDescription "MSI"

  Invoke-CheckedProcess -FilePath $nsis.FullName -ArgumentList "/S /D=$nsisRoot"
  $nsisApplication = Get-ChildItem -LiteralPath $nsisRoot -Recurse -File -Filter "code-quest-advance.exe" | Select-Object -First 1
  if ($null -eq $nsisApplication) {
    throw "NSIS installation did not contain code-quest-advance.exe"
  }
  Test-ApplicationStartup -ApplicationPath $nsisApplication.FullName -PackageDescription "NSIS"

  $uninstaller = Get-ChildItem -LiteralPath $nsisRoot -Recurse -File -Filter "uninstall*.exe" | Select-Object -First 1
  if ($null -ne $uninstaller) {
    Invoke-CheckedProcess -FilePath $uninstaller.FullName -ArgumentList "/S"
  }
} finally {
  if (Test-Path -LiteralPath $testRoot) {
    Remove-Item -LiteralPath $testRoot -Recurse -Force
  }
}

Write-Host "PASS: uploaded MSI and NSIS packages passed checksum, extraction, and startup tests"
