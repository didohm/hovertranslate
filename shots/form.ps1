# Puts the sample form on top so the pointer can rest on real text.
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$form = New-Object System.Windows.Forms.Form
$form.Text = "Sample text for HoverTranslate"
$form.StartPosition = "Manual"
$form.Location = New-Object System.Drawing.Point(520, 60)
$form.Size = New-Object System.Drawing.Size(620, 260)
$form.BackColor = [System.Drawing.Color]::White
$form.TopMost = $true

$box = New-Object System.Windows.Forms.TextBox
$box.Multiline = $true
$box.Location = New-Object System.Drawing.Point(20, 50)
$box.Size = New-Object System.Drawing.Size(560, 170)
$box.Font = New-Object System.Drawing.Font("Segoe UI", 14)
$box.Text = "El rapido zorro marron salta sobre el perro perezoso. La traduccion necesita internet."
$form.Controls.Add($box)

$form.Show()
$form.Activate()
[void]$form.TopMost
Start-Sleep -Seconds 30
$form.Close()
