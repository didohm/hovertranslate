Add-Type @"
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class Win7 {
  public delegate bool EnumProc(IntPtr hwnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public static List<string> Found = new List<string>();
  public static void Dump() {
    EnumWindows((h, l) => {
      StringBuilder sb = new StringBuilder(256); GetWindowText(h, sb, 256);
      string t = sb.ToString();
      if (t.Contains("HoverTranslate")) {
        RECT r; GetWindowRect(h, out r);
        Found.Add(t + " vis=" + IsWindowVisible(h) + " rect=" + r.Left + "," + r.Top + "," + r.Right + "," + r.Bottom);
      }
      return true;
    }, IntPtr.Zero);
  }
}
"@
Start-Sleep -Seconds 10
[Win7]::Dump()
[Win7]::Found
