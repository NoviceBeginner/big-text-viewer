# Big Text Viewer V4

**產品名稱：** Big Text Viewer  
**產品版本：** V4  
**著作權：** ChunChun

Big Text Viewer V4 是以 Rust、egui/eframe 製作的大型文字檔檢視器，支援最大 4GB 檔案、繁體中文顯示、主搜尋、黃色關鍵字高亮、多階段結果篩選，以及 Nil result 畫面。

## V4 改進

- 內嵌 `Noto Sans CJK TC` 繁體中文字型，不依賴目標電腦是否安裝中文字型。
- 支援 UTF-8、UTF-8 BOM、UTF-16 LE/BE，並利用 `chardetng` 偵測 Big5/HKSCS 等舊式編碼。
- Windows EXE 內嵌版本資源：產品名稱 `Big Text Viewer`、產品版本 `V4`、著作權 `ChunChun`。
- Windows manifest 使用 `asInvoker`，避免程式被誤判為安裝程式而不必要地要求系統管理員權限。
- 預設使用 Glow/OpenGL 渲染，以提高不同 Windows 顯示卡環境的相容性。
- Windows MSVC x64 release 建置採用靜態 CRT 設定，減少目標電腦缺少執行階段 DLL 的問題。

## Windows 建置

需求：

1. Windows 10/11 x64。
2. 安裝 Rust MSVC toolchain。
3. 安裝 Visual Studio Build Tools，選取「使用 C++ 的桌面開發」及 Windows SDK。

在專案根目錄執行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-windows-x64.ps1
```

輸出：`dist\Big Text Viewer V4.exe`

## 數位簽章

Windows manifest、版本資料與靜態 CRT 只能改善相容性，**不能取代可信任的 Authenticode 程式碼簽章**。公開派發時，應使用可信 CA 簽發的 OV/EV Code Signing 憑證、Microsoft Artifact Signing，或透過 Microsoft Store 發佈。

使用 PFX 憑證簽署：

```powershell
$pw = Read-Host "PFX password" -AsSecureString
.\scripts\sign-windows.ps1 -CertificatePath "C:\cert\publisher.pfx" -CertificatePassword $pw
```

簽章腳本會使用 SHA-256、RFC 3161 timestamp，並於完成後驗證簽章。請勿把 PFX 憑證或密碼加入專案或 ZIP。

注意：自簽憑證不會自動取得公眾電腦信任；沒有真正的私人金鑰與可信憑證時，無法在打包階段替 EXE 產生有效發行者認證。新簽章憑證仍可能需要時間累積 SmartScreen reputation。

## 使用方式

1. 啟動程式後拖曳檔案到視窗，或按「開啟檔案」/ `Ctrl+O`。
2. 輸入關鍵字，按 Enter 或「搜尋」。
3. 有結果時，原文中的命中文字會以黃色高亮，全部命中行會顯示在新的結果視窗。
4. 在結果視窗輸入另一個關鍵字並按「進一步篩選」，可不限次數建立下一階段結果。
5. 沒有結果時會顯示 `Nil result`。
6. `F3` 前往下一結果，`Shift+F3` 前往上一結果，`Esc` 清除搜尋。

## 字型授權

專案內的 Noto Sans CJK TC 字型依 SIL Open Font License 1.1 發佈，完整授權見 `assets/LICENSE-NOTO.txt`。
