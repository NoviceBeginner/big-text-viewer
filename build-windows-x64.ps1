$ErrorActionPreference = "Stop"

Write-Host "Building Big Text Viewer V4 for Windows x64 (MSVC)..."
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc

New-Item -ItemType Directory -Force -Path "dist" | Out-Null
Copy-Item "target\x86_64-pc-windows-msvc\release\big-text-viewer.exe" "dist\Big Text Viewer V4.exe" -Force
Copy-Item "README.md" "dist\README.md" -Force
Copy-Item "assets\LICENSE-NOTO.txt" "dist\LICENSE-NOTO.txt" -Force

Write-Host "Build complete: dist\Big Text Viewer V4.exe"
Write-Host "For public distribution, sign the EXE with scripts\sign-windows.ps1."
