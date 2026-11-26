# Dot-source this script to activate the Windows tools in the current shell.
$settings = Join-Path $PSScriptRoot '../.private/dev.json'
if (Test-Path -LiteralPath $settings) {
    $local = Get-Content -LiteralPath $settings -Raw | ConvertFrom-Json
    if ($local.toolHome) {
        $env:RUSTUP_HOME = Join-Path $local.toolHome 'rustup'
        $env:CARGO_HOME = Join-Path $local.toolHome 'cargo'
        $env:CARGO_TARGET_DIR = Join-Path $local.toolHome 'target'
        $env:Path = "$(Join-Path $env:CARGO_HOME 'bin');$env:Path"
    }
    if ($local.postgresBin) { $env:Path = "$($local.postgresBin);$env:Path" }
}
if (!$env:OPENCODE_BIN -and !(Get-Command opencode -ErrorAction SilentlyContinue)) {
    $desktopCli = Get-Process opencode-cli -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Path
    if ($desktopCli) { $env:OPENCODE_BIN = $desktopCli }
}
