Add-Type @"
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class Win {
  public delegate bool EnumProc(IntPtr hwnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int t, bool repaint);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  public static List<string> Titles = new List<string>();
  public static IntPtr Find(string title) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      if (!IsWindowVisible(h)) return true;
      StringBuilder sb = new StringBuilder(256);
      GetWindowText(h, sb, 256);
      if (sb.ToString() == title) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@
$h = [Win]::Find("HoverTranslate")
if ($h -eq [IntPtr]::Zero) { "app window not found" } else { "moving app window"; [void][Win]::MoveWindow($h, 0, 0, 420, 640, $true) }
Start-Sleep -Seconds 2
# Over the sample text box of the form (form at 520,60; text box at +20,+50)
[void][Win]::SetCursorPos(620, 130)
Start-Sleep -Seconds 14
# Move away so the popup should close again
[void][Win]::SetCursorPos(60, 60)
Start-Sleep -Seconds 3
"done"
