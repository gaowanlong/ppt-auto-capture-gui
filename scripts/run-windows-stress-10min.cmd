@echo off
setlocal
powershell.exe -NoProfile -File "%~dp0run-windows-stress.ps1" -Minutes 10 -OutputDirectory "%~dp0stress-results"
set "stress_exit=%ERRORLEVEL%"
echo Reports are in the stress-results folder next to this launcher.
pause
exit /b %stress_exit%
