@echo off
:: Build and launch the simulation in release mode (Windows).
cd /d "%~dp0"

where cargo >nul 2>nul || set "PATH=%PATH%;%USERPROFILE%\.cargo\bin"

:: DirectX 12 is the most reliable wgpu backend on Windows
if not defined WGPU_BACKEND set WGPU_BACKEND=dx12

cargo run --release
