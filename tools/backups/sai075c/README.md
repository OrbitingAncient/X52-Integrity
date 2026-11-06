# Logitech sai075c export

This directory is for a **local** copy of the 2017 Logitech X52 package (`sai075c.inf` plus catalog and binaries). Those files are Logitech copyright and are **not** published in this repository.

Export them on a machine that still has the Logitech driver installed before switching to inbox HID:

```powershell
pnputil /export-driver oemXXX.inf tools\backups\sai075c
```

Restore (administrator, after you confirm):

```powershell
.\tools\restore-saitek-x52-driver.ps1
```

or the GUI **Logitech driver** button.
