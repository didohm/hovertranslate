Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class Win4 {
  public delegate bool EnumProc(IntPtr hwnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public static IntPtr Find(string t) { IntPtr f = IntPtr.Zero; EnumWindows((h,l)=>{ StringBuilder sb=new StringBuilder(256); GetWindowText(h,sb,256); if(sb.ToString()==t){f=h;return false;} return true;}, IntPtr.Zero); return f; }
  public static string Info(IntPtr h) {
    StringBuilder sb = new StringBuilder(256); GetWindowText(h, sb, 256);
    RECT r; GetWindowRect(h, out r);
    return sb.ToString() + " vis=" + IsWindowVisible(h) + " rect=" + r.Left + "," + r.Top + "," + r.Right + "," + r.Bottom;
  }
}
"@

# 1. Start notepad with the sample text
$np = Start-Process notepad -ArgumentList "shots\sample.txt" -PassThru
Start-Sleep -Seconds 6
$h = [Win4]::Find("sample.txt - Notepad")
if ($h -eq [IntPtr]::Zero) {
  $h = [Win4]::Find("sample.txt")
  if ($h -eq [IntPtr]::Zero) { "notepad window not found" } else { [Win4]::Info($h) }
} else {
  [Win4]::Info($h)
}
Start-Sleep -Seconds 1
# 2. hover over the first line of text
[void][Win4]::SetCursorPos(300, 250)
Start-Sleep -Seconds 12
[void][Win4]::SetCursorPos(60, 60)
Start-Sleep -Seconds 2
"done"
