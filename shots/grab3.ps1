# Puts the pointer on a piece of text and takes a picture of the screen.
param([int]$X = 120, [int]$Y = 428)
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class MB {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
}
"@

[void][MB]::SetCursorPos($X, $Y)
Start-Sleep -Seconds 7
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
$bitmap.Save((Join-Path $PSScriptRoot "screen3.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
"captured"
