# Windows-only local test cluster. Credentials use current-user DPAPI protection.
param([ValidateSet('start', 'stop')][string]$Action = 'start')
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:LOCALAPPDATA 'Rustday\postgres-dev'
$data = Join-Path $root 'data'
$port = 55437
$ctl = (Get-Command pg_ctl.exe -ErrorAction Stop).Source
if ($Action -eq 'stop') {
    & $ctl -D $data -w stop
    if ($LASTEXITCODE) { throw 'Could not stop the boardcraft cluster.' }
    Remove-Item Env:RUSTDAY_TEST_DATABASE_URL -ErrorAction SilentlyContinue
    return
}
New-Item -ItemType Directory -Force $root | Out-Null
function Get-LocalPassword($role) {
    $file = Join-Path $root "$role.protected"
    if (!(Test-Path $file)) {
        if (Test-Path (Join-Path $data 'PG_VERSION')) { throw "Missing protected credential for $role; inspect the existing cluster." }
        $bytes = New-Object byte[] 32
        $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
        try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
        $secret = [BitConverter]::ToString($bytes).Replace('-', '') | ConvertTo-SecureString -AsPlainText -Force
        ConvertFrom-SecureString $secret | Set-Content -LiteralPath $file
    }
    $secure = Get-Content -LiteralPath $file | ConvertTo-SecureString
    return [Net.NetworkCredential]::new('', $secure).Password
}
$admin = Get-LocalPassword 'rustday_admin'
$app = Get-LocalPassword 'rustday_test'
if (!(Test-Path (Join-Path $data 'PG_VERSION'))) {
    $pwfile = Join-Path $root ([IO.Path]::GetRandomFileName())
    try {
        [IO.File]::WriteAllText($pwfile, $admin, [Text.Encoding]::ASCII)
        & initdb.exe -D $data -U rustday_admin "--pwfile=$pwfile" -A scram-sha-256 -E UTF8 --locale=C
        if ($LASTEXITCODE) { throw 'Cluster initialization failed.' }
    } finally { Remove-Item -LiteralPath $pwfile -ErrorAction SilentlyContinue }
}
& $ctl -D $data status | Out-Null
if ($LASTEXITCODE) {
    & $ctl -D $data -l (Join-Path $root 'server.log') -o "-h 127.0.0.1 -p $port" -w start
    if ($LASTEXITCODE) { throw 'Could not start the local cluster. Check server.log and the port.' }
}
$savedPassword = $env:PGPASSWORD
try {
    $env:PGPASSWORD = $admin
    $flags = @('-X', '-w', '-qAt', '-v', 'ON_ERROR_STOP=1', '-h', '127.0.0.1', '-p', $port, '-U', 'rustday_admin', '-d', 'postgres')
    $serverData = & psql.exe @flags -c 'SHOW data_directory'
    if ($LASTEXITCODE -or !$serverData -or [IO.Path]::GetFullPath($serverData) -ne [IO.Path]::GetFullPath($data)) { throw 'Unexpected database instance.' }
    @"
SELECT 'CREATE ROLE rustday_test LOGIN PASSWORD ''$app''' WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'rustday_test') \gexec
SELECT 'CREATE DATABASE rustday_test OWNER rustday_test' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'rustday_test') \gexec
"@ | & psql.exe @flags
    if ($LASTEXITCODE) { throw 'Test database initialization failed.' }
    $env:PGPASSWORD = $app
    & psql.exe -X -w -h 127.0.0.1 -p $port -U rustday_test -d rustday_test -c 'SELECT current_database(), current_user'
    if ($LASTEXITCODE) { throw 'Test login failed.' }
    $env:RUSTDAY_TEST_DATABASE_URL = "postgresql://rustday_test:${app}@127.0.0.1:${port}/rustday_test"
} finally { $env:PGPASSWORD = $savedPassword }
