param(
    [Parameter(Mandatory = $true)]
    [string]$Binary,

    [Parameter(Mandatory = $true)]
    [string]$CertificateBase64,

    [Parameter(Mandatory = $true)]
    [string]$CertificatePassword,

    [Parameter(Mandatory = $true)]
    [string]$TimestampUrl
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Binary)) {
    throw "Windows binary was not found: $Binary"
}

$signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
if (-not $signtool) {
    throw "signtool.exe was not found on PATH"
}

$certificatePath = Join-Path $env:RUNNER_TEMP "llmff-binary-codesign.p12"
$securePassword = ConvertTo-SecureString -String $CertificatePassword -AsPlainText -Force
$certificate = $null

try {
    [System.Convert]::FromBase64String($CertificateBase64) |
        Set-Content -LiteralPath $certificatePath -AsByteStream

    $certificate = Import-PfxCertificate `
        -FilePath $certificatePath `
        -CertStoreLocation Cert:\CurrentUser\My `
        -Password $securePassword

    if (-not $certificate.Thumbprint) {
        throw "imported Windows signing certificate has no thumbprint"
    }

    & $signtool.Source sign `
        /fd SHA256 `
        /td SHA256 `
        /tr $TimestampUrl `
        /sha1 $certificate.Thumbprint `
        $Binary
    if ($LASTEXITCODE -ne 0) {
        throw "signtool sign failed with exit code $LASTEXITCODE"
    }

    & $signtool.Source verify /pa /v $Binary
    if ($LASTEXITCODE -ne 0) {
        throw "signtool verify failed with exit code $LASTEXITCODE"
    }
}
finally {
    if ($certificate -and $certificate.Thumbprint) {
        Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($certificate.Thumbprint)" -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $certificatePath -ErrorAction SilentlyContinue
}

