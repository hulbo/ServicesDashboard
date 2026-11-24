# 시험용 앱 창(이 PID의 창)만 최소화한다. 다른 창은 건드리지 않는다.
# 첫 표시 때 창이 다시 보일 수 있어, 최소화 상태가 연달아 확인될 때까지 다시 적용한다.
param([int]$ProcId)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class SdE2eWindow {
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
}
"@
$p = Get-Process -Id $ProcId
for ($i = 0; $i -lt 50 -and $p.MainWindowHandle -eq 0; $i++) { Start-Sleep -Milliseconds 100; $p.Refresh() }
$h = $p.MainWindowHandle
$ok = 0
for ($i = 0; $i -lt 60 -and $ok -lt 6; $i++) {
  if ([SdE2eWindow]::IsIconic($h)) { $ok++ } else { $ok = 0; [SdE2eWindow]::ShowWindow($h, 7) | Out-Null }  # SW_SHOWMINNOACTIVE
  Start-Sleep -Milliseconds 150
}
"iconic=$([SdE2eWindow]::IsIconic($h))"
