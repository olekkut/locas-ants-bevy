@echo off
cd /d "%~dp0"
set WGPU_BACKEND=dx12
echo Starting Bevy Ant Simulation...
"%USERPROFILE%\.cargo\target-cache\optimistic-carson\release\optimistic-carson.exe"
