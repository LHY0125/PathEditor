@echo off

echo Building Installer...
"D:\Program Files (x86)\Inno Setup 6\ISCC.exe" "dist\installer.iss"

if %ERRORLEVEL% NEQ 0 (
    echo Installer build failed!
    exit /b %ERRORLEVEL%
)

echo Done! Installer is in dist\dist\
pause