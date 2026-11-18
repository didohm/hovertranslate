# Moves the mouse over the app window so the hover watcher has something to read.
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Mouse {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string cls, string name);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$hwnd = [Mouse]::FindWindow($null, "HoverTranslate")
if ($hwnd -eq [IntPtr]::Zero) { "window not found"; exit 1 }
$rect = New-Object Mouse+RECT
[void][Mouse]::GetWindowRect($hwnd, [ref]$rect)
"window at $($rect.Left),$($rect.Top) - $($rect.Right),$($rect.Bottom)"
[void][Mouse]::SetCursorPos($rect.Left + 60, $rect.Top + 185)
Start-Sleep -Seconds 6
"done"
