@echo off
cd /d "%~dp0"
title Join a game
echo Join a game run by someone else.
echo.
set "NAME="
set /p NAME=Your name (press Enter to use your Windows name): 

:ask
echo.
set "ADDR="
set /p ADDR=Host address, like 203.0.113.5 or 203.0.113.5:7777 (port 7777 if you leave it out): 
if "%ADDR%"=="" goto ask
if "%ADDR::=%"=="%ADDR%" set "ADDR=%ADDR%:7777"

echo.
echo Joining %ADDR% ...
if "%NAME%"=="" (
    gamedemo.exe join "%ADDR%"
) else (
    gamedemo.exe join "%ADDR%" "%NAME%"
)
if errorlevel 1 goto failed
exit /b 0

:failed
echo.
echo That did not work. Check the address and that the host is running, then try again.
pause
goto ask
