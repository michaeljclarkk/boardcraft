# Return only text and geometry from this one captured emulator frame.
param([Parameter(Mandatory)][string]$Image)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$file = [Windows.Storage.StorageFile, Windows.Storage, ContentType=WindowsRuntime]
$stream = [Windows.Storage.Streams.IRandomAccessStream, Windows.Storage.Streams, ContentType=WindowsRuntime]
$decoder = [Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics.Imaging, ContentType=WindowsRuntime]
$bitmap = [Windows.Graphics.Imaging.SoftwareBitmap, Windows.Graphics.Imaging, ContentType=WindowsRuntime]
$ocr = [Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType=WindowsRuntime]
$result = [Windows.Media.Ocr.OcrResult, Windows.Foundation, ContentType=WindowsRuntime]
$language = [Windows.Globalization.Language, Windows.Globalization, ContentType=WindowsRuntime]
$asTask = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
    $_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetParameters().Count -eq 1 -and
    $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
} | Select-Object -First 1
function Await-Operation($operation, $type) {
    $task = $asTask.MakeGenericMethod($type).Invoke($null, @($operation))
    $task.GetAwaiter().GetResult()
}
$engine = $ocr::TryCreateFromLanguage($language::new('en-US'))
if (!$engine) { throw 'English Windows OCR language pack required' }
$f = Await-Operation ($file::GetFileFromPathAsync((Resolve-Path -LiteralPath $Image).Path)) $file
$s = Await-Operation ($f.OpenAsync([Windows.Storage.FileAccessMode]::Read)) $stream
$d = Await-Operation ($decoder::CreateAsync($s)) $decoder
$b = Await-Operation ($d.GetSoftwareBitmapAsync()) $bitmap
$r = Await-Operation ($engine.RecognizeAsync($b)) $result
$lines = @(foreach ($line in $r.Lines) {
    $first = ($line.Words | Select-Object -First 1).BoundingRect
    @{ text = $line.Text; x = [int]($first.X + $first.Width/2); y = [int]($first.Y + $first.Height/2) }
})
ConvertTo-Json -Compress -InputObject $lines
