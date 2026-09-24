# Code signing policy

Free code signing provided by SignPath.io,
certificate by SignPath Foundation.

## Project

- Project: Big Text Viewer
- Maintainer: ChunChun
- Repository: https://github.com/你的帳戶/big-text-viewer
- License: MIT License

## Team roles

- Author: ChunChun
- Committer: ChunChun
- Reviewer: ChunChun
- Signing approver: ChunChun

## Build process

Official Windows binaries are built from this public repository by
GitHub Actions using GitHub-hosted Windows runners.

Release binaries are submitted directly from the GitHub Actions workflow
to SignPath. Locally compiled binaries are not eligible for official
release signing.

Every production signing request requires manual approval.

## Privacy policy

Big Text Viewer does not transfer information to networked systems unless
specifically requested by the user or the person installing or operating it.

Full privacy policy:
https://github.com/你的帳戶/big-text-viewer/blob/main/docs/privacy-policy.md

## Verification

Official releases can be verified with:

```powershell
Get-AuthenticodeSignature ".\Big Text Viewer.exe" |
    Format-List Status, SignerCertificate, TimeStamperCertificate
```
