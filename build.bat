@echo off
setlocal
cd /d "%~dp0"

echo [1/2] Building release binary...
cargo build --release
if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Build failed!
    timeout /t 3 2>nul
    pause
    exit /b %ERRORLEVEL%
)

echo [2/2] Updating ztrim.exe in workspace root...
copy /y "target\release\ztrim.exe" "ztrim.exe" >nul
if %ERRORLEVEL% neq 0 (
    echo.
    echo [WARNING] Could not overwrite root ztrim.exe - it might be currently running.
    echo You can still find the built executable at: target\release\ztrim.exe
) else (
    echo [SUCCESS] Build complete! Executable updated at ztrim.exe
)

echo.
timeout /t 3 2>nul
