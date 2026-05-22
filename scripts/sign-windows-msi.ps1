param(
    [Parameter(Mandatory = $true)]
    [string]$Msi,
    [Parameter(Mandatory = $true)]
    [string]$CertificateBase64,
    [Parameter(Mandatory = $true)]
    [string]$CertificatePassword,
    [Parameter(Mandatory = $true)]
    [string]$TimestampUrl
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Msi -PathType Leaf)) {
    throw "MSI does not exist: $Msi"
}

$signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
if (-not $signtool) {
    throw "signtool.exe was not found on PATH"
}

$certificatePath = Join-Path $env:RUNNER_TEMP "llmff-codesign.p12"
$certificate = $null

try {
    [IO.File]::WriteAllBytes($certificatePath, [Convert]::FromBase64String($CertificateBase64))
    $securePassword = ConvertTo-SecureString -String $CertificatePassword -AsPlainText -Force
    $certificate = Import-PfxCertificate `
        -FilePath $certificatePath `
        -CertStoreLocation Cert:\CurrentUser\My `
        -Password $securePassword

    if (-not $certificate.Thumbprint) {
        throw "imported certificate has no thumbprint"
    }

    & $signtool.Source sign `
        /fd SHA256 `
        /td SHA256 `
        /tr $TimestampUrl `
        /sha1 $certificate.Thumbprint `
        $Msi

    if ($LASTEXITCODE -ne 0) {
        throw "signtool sign failed with exit code $LASTEXITCODE"
    }

    & $signtool.Source verify /pa /v $Msi

    if ($LASTEXITCODE -ne 0) {
        throw "signtool verify failed with exit code $LASTEXITCODE"
    }
} finally {
    if ($certificate -and $certificate.Thumbprint) {
        Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($certificate.Thumbprint)" -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $certificatePath -ErrorAction SilentlyContinue
}
