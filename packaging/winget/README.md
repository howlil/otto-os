# WinGet Manifest Templates

These manifests are template files for `howlil.OttoOS`.

Before submitting a PR to `microsoft/winget-pkgs`, stamp `PackageVersion`, `InstallerUrl`, and `InstallerSha256` from the published GitHub Release asset (done automatically in `.github/workflows/release.yml` when tagging a release, producing ready-to-submit manifests as release assets):

```powershell
winget validate --manifest .\packaging\winget
```
