Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class Win6 {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  public static void Screen(string path) {
    var b = System.Windows.Forms.Screen.PrimaryScreen.Bounds;
    using (var bmp = new System.Drawing.Bitmap(b.Width, b.Height))
    using (var g = System.Drawing.Graphics.FromImage(bmp)) {
      g.CopyFromScreen(b.Location, System.Drawing.Point.Empty, b.Size);
      bmp.Save(path, System.Drawing.Imaging.ImageFormat.Png);
    }
  }
}
"@ -ReferencedAssemblies System.Windows.Forms, System.Drawing

[void][Win6]::SetCursorPos(693, 320)
Start-Sleep -Seconds 9
[Win6]::Screen((Join-Path $PSScriptRoot "screen.png"))
Start-Sleep -Seconds 1
"captured"
