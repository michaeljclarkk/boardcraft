param([Parameter(Mandatory)][string]$OutputApk)
$ErrorActionPreference = 'Stop'

function Invoke-Checked($tool) {
    & $tool @args
    if ($LASTEXITCODE -ne 0) { throw "$tool exited with code $LASTEXITCODE" }
}

if (-not [IO.Path]::IsPathRooted($OutputApk) -or (Test-Path -LiteralPath $OutputApk)) {
    throw 'OutputApk must be a new absolute file path'
}
$tools = Join-Path $env:ANDROID_HOME 'build-tools/35.0.0'
$platform = Join-Path $env:ANDROID_HOME 'platforms/android-35/android.jar'
$keytool = if ($env:JAVA_HOME) { Join-Path $env:JAVA_HOME 'bin/keytool.exe' } else { 'keytool.exe' }
$manifest = Join-Path $PSScriptRoot 'Cargo.toml'
$abi = 'x86_64-linux-android'

# Compile just the platform's matching, transparent IME proxy from the locked
# Cargo checkout. Its nativeIme JNI method belongs to this same mobile host.
$so = $null
$proxy = $null
Invoke-Checked cargo build --manifest-path $manifest --locked --target $abi --lib --message-format=json-render-diagnostics | ForEach-Object {
    $artifact = $_ | ConvertFrom-Json
    if ($artifact.reason -eq 'compiler-artifact') {
        if ($artifact.target.name -eq 'gpui_mobile') {
            $proxy = Join-Path (Split-Path $artifact.manifest_path) 'example/android/gradle/app/src/main/java/dev/gpui/mobile/GpuiInputActivity.java'
        } elseif ($artifact.target.name -eq 'boardcraft_kit_fixture') {
            $so = $artifact.filenames | Where-Object { [IO.Path]::GetFileName($_) -eq 'libboardcraft_kit_fixture.so' }
        }
    }
}
if (!$so -or !$proxy) { throw 'Missing fixture or Android host build artifact' }

# Everything except the final signed APK (including its one-build debug key)
# lives in this invocation's staging directory.
# Stage beside the requested APK: the caller owns this output location and
# publishing the verified file is a same-volume move.
$work = Join-Path ([IO.Path]::GetDirectoryName($OutputApk)) ('boardcraft-apk-' + [guid]::NewGuid())
$payload = Join-Path $work 'payload'
$lib = Join-Path $payload 'lib/x86_64'
$unsigned = Join-Path $work 'unsigned.apk'
$aligned = Join-Path $work 'aligned.apk'
$signed = Join-Path $work 'signed.apk'
$key = Join-Path $work 'fixture.p12'
[void](New-Item -Path $work -ItemType Directory -ErrorAction Stop)
$buildFailed = $false
try {
    [void][IO.Directory]::CreateDirectory($lib)
    Copy-Item -LiteralPath $so -Destination $lib
    $classes = [IO.Directory]::CreateDirectory((Join-Path $work 'classes')).FullName
    $javac = if ($env:JAVA_HOME) { Join-Path $env:JAVA_HOME 'bin/javac.exe' } else { 'javac.exe' }
    Invoke-Checked $javac --release 8 -classpath $platform -encoding UTF-8 -d $classes $proxy (Join-Path $PSScriptRoot 'java/SessionStore.java')
    $bytecode = @(Get-ChildItem -LiteralPath $classes -Filter '*.class' -Recurse | ForEach-Object FullName)
    Invoke-Checked (Join-Path $tools 'd8.bat') --min-api 28 --lib $platform --output $payload @bytecode
    Invoke-Checked (Join-Path $tools 'aapt.exe') package -M (Join-Path $PSScriptRoot 'AndroidManifest.xml') -I $platform -F $unsigned '-0' so $payload
    Invoke-Checked (Join-Path $tools 'zipalign.exe') -P 16 4 $unsigned $aligned
    Invoke-Checked $keytool -genkeypair -keystore $key -storetype PKCS12 -storepass android -keypass android -alias fixture -keyalg RSA -keysize 2048 -validity 30 -dname 'CN=Boardcraft Debug Fixture'
    $signer = Join-Path $tools 'apksigner.bat'
    Invoke-Checked $signer sign --ks $key --ks-pass pass:android --ks-key-alias fixture --out $signed $aligned
    Invoke-Checked $signer verify $signed
    [IO.File]::Move($signed, $OutputApk)
} catch {
    $buildFailed = $true
    throw
} finally {
    try { [IO.Directory]::Delete($work, $true) }
    catch {
        if (!$buildFailed) { throw }
        Write-Warning "Could not clean APK staging directory ${work}: $_"
    }
}
