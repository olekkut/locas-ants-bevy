@echo off
:: Build and launch the simulation in release mode (Windows).
cd /d "%~dp0"

where cargo >nul 2>nul || set "PATH=%PATH%;%USERPROFILE%\.cargo\bin"

cargo run --release
