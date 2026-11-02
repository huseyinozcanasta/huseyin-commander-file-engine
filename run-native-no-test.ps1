# Only validates our own temporary test program's Windows NO button.
# It cannot authorize or edit arbitrary user files.
$ErrorActionPreference="Stop"
$root=Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root
$env:PATH=(Join-Path $env:USERPROFILE ".cargo\bin")+";"+$env:PATH
if(-not $env:CARGO_TARGET_DIR){$env:CARGO_TARGET_DIR=Join-Path $root "target"}
$ErrorActionPreference="Continue"
& cargo build --locked --offline --bin native_no_dialog *> (Join-Path $root "native-build.log")
$buildExit=$LASTEXITCODE
$ErrorActionPreference="Stop"
if($buildExit -ne 0){Get-Content .\native-build.log -Tail 35;throw "Native dialog build failed"}
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class HCConsentTest {
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr dlg,int id);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd,uint msg,IntPtr w,IntPtr l);
}
'@
$exe=Join-Path $env:CARGO_TARGET_DIR "debug\native_no_dialog.exe"
$stdout=Join-Path $root "native-no.log"
$stderr=Join-Path $root "native-no-errors.log"
$p=Start-Process -FilePath $exe -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
try {
 $win=$null
 for($i=0;$i -lt 40 -and $null -eq $win;$i++){
  Start-Sleep -Milliseconds 200
  $cond=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,[int]$p.Id)
  $win=[System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children,$cond)
 }
 if($null -eq $win -or $win.Current.Name -ne "HC-A16 Native Consent Test"){throw "Expected Windows consent dialog was not found"}
 $no=[HCConsentTest]::GetDlgItem([IntPtr]$win.Current.NativeWindowHandle,7)
 if($no -eq [IntPtr]::Zero){throw "Native IDNO 7 button not found"}
 [void][HCConsentTest]::SendMessageW($no,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)
 [void]$p.WaitForExit(5000)
 if(-not $p.HasExited){throw "Dialog process did not finish"}
 $out=Get-Content -LiteralPath $stdout -Raw
 $p.Refresh()
 if(-not $p.HasExited -or $out -notmatch "NATIVE_DIALOG_RESULT=7" -or $out -notmatch "A16_NATIVE_NO_DENIED_AND_NO_MUTATION"){
  throw "Negative native consent outcome failed: $out"
 }
 Write-Output "A16_NATIVE_WINDOWS_NO_DECLINED_PASSED"
} finally {
 $p.Refresh()
 if(-not $p.HasExited){Stop-Process -Id $p.Id -Force}
}
