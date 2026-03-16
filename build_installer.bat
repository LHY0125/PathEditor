@echo off
echo Copying DLLs...
if not exist bin mkdir bin
copy /Y "libs\iup-3.31_Win64_dllw6_lib\*.dll" bin\

echo Building Installer...
"D:\Program Files (x86)\Inno Setup 6\ISCC.exe" "dist\installer.iss"

if %ERRORLEVEL% NEQ 0 (
    echo Installer build failed!
    exit /b %ERRORLEVEL%
)

echo Done! Installer is in dist\dist\
pause