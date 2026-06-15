@echo off
:: Add Cargo to PATH if it's not already there
set PATH=%PATH%;%USERPROFILE%\.cargo\bin

:: Set target directory outside of Syncthing synced folder
set CARGO_TARGET_DIR=%USERPROFILE%\.cargo\target-cache\optimistic-carson

:: Force DirectX 12 backend to avoid Vulkan deadlock
set WGPU_BACKEND=dx12

echo Stopping any running instances of optimistic-carson.exe...
taskkill /IM optimistic-carson.exe /F 2>nul

echo Compiling the application...
cargo build

echo Deploying assets to target directory...
xcopy /E /I /Y "assets" "%CARGO_TARGET_DIR%\debug\assets"

echo Running the application...
cargo run
