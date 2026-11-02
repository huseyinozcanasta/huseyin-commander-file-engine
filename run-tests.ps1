$ErrorActionPreference="Stop"
$root=Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root
$env:PATH=(Join-Path $env:USERPROFILE ".cargo\bin")+";"+$env:PATH
if(-not $env:CARGO_TARGET_DIR){$env:CARGO_TARGET_DIR=Join-Path $root "target"}
$env:CARGO_INCREMENTAL="0"
$env:CARGO_BUILD_JOBS="1"
$ErrorActionPreference="Continue"
& cargo test --locked --offline -- --test-threads=1 *> (Join-Path $root "a16-tests.log")
$testExit=$LASTEXITCODE
& cargo clippy --locked --offline --all-targets -- -D warnings *> (Join-Path $root "a16-clippy.log")
$lintExit=$LASTEXITCODE
$ErrorActionPreference="Stop"
if($testExit -ne 0){Get-Content .\a16-tests.log -Tail 60;throw "A16 tests failed"}
if($lintExit -ne 0){Get-Content .\a16-clippy.log -Tail 60;throw "A16 clippy failed"}
Get-Content .\a16-tests.log -Tail 40
Write-Output "A16_WINDOWS_TESTS_AND_CLIPPY_PASSED"
