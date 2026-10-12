@echo off
cd /d "%~dp0"
echo Hosting a game on port 7777. Leave this window open; close it (or press Ctrl+C) to save and stop.
gamedemo-host.exe 7777
echo.
pause
