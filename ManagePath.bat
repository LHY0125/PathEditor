@echo off
chcp 65001 >nul
setlocal EnableDelayedExpansion

REM 检查管理员权限
>nul 2>&1 "%SYSTEMROOT%\system32\cacls.exe" "%SYSTEMROOT%\system32\config\system"
if '%errorlevel%' NEQ '0' (
    echo 正在请求管理员权限...
    echo [注意] 将会弹出一个新窗口进行操作，请留意任务栏或 UAC 提示。
    echo 本窗口即将关闭...
    timeout /t 3 >nul
    goto UACPrompt
) else ( goto gotAdmin )

:UACPrompt
    echo Set UAC = CreateObject^("Shell.Application"^) > "%temp%\getadmin.vbs"
    set params = %*:"=""
    echo UAC.ShellExecute "cmd.exe", "/c ""%~s0"" %params%", "", "runas", 1 >> "%temp%\getadmin.vbs"
    "%temp%\getadmin.vbs"
    del "%temp%\getadmin.vbs"
    exit /B

:gotAdmin
    pushd "%~dp0"

:menu
cls
echo ========================================================
echo             系统 PATH 环境变量管理器
echo ========================================================
echo 1. 导出当前 PATH 到 'path_list.txt' (用于编辑)
echo 2. 从 'path_list.txt' 导入 PATH (应用更改)
echo 3. 备份当前 PATH 到带时间戳的文件
echo 4. 退出
echo ========================================================
set /p choice=请输入选项 (1-4): 

if "%choice%"=="1" goto export
if "%choice%"=="2" goto import
if "%choice%"=="3" goto backup
if "%choice%"=="4" goto end
goto menu

:export
cls
echo 正在导出 PATH 到 path_list.txt...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$p = [Environment]::GetEnvironmentVariable('PATH', 'Machine'); if ($p) { $p -split ';' | Where-Object { $_.Trim() -ne '' } | Out-File 'path_list.txt' -Encoding utf8; Write-Host '导出完成！您现在可以编辑 path_list.txt' -ForegroundColor Green } else { Write-Host '错误：无法获取 PATH 变量。' -ForegroundColor Red }"
pause
goto menu

:import
cls
if not exist "path_list.txt" (
    echo 错误：未找到 'path_list.txt'！请先导出或手动创建该文件。
    pause
    goto menu
)
echo 正在从 path_list.txt 导入 PATH...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$lines = Get-Content 'path_list.txt' | Where-Object { $_.Trim() -ne '' }; if ($lines.Count -eq 0) { Write-Host '错误：path_list.txt 为空！' -ForegroundColor Red; exit }; $newPath = $lines -join ';'; Write-Host ('新的 PATH 将包含 ' + $lines.Count + ' 个条目。'); try { [Environment]::SetEnvironmentVariable('PATH', $newPath, 'Machine'); Write-Host 'PATH 更新成功！' -ForegroundColor Green } catch { Write-Host ('更新 PATH 时出错：' + $_.Exception.Message) -ForegroundColor Red }"
echo.
echo 请重启您的终端/应用程序以使更改生效。
pause
goto menu

:backup
cls
echo 正在备份 PATH...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$date = Get-Date -Format 'yyyyMMdd_HHmmss'; $outFile = 'path_backup_' + $date + '.txt'; $p = [Environment]::GetEnvironmentVariable('PATH', 'Machine'); if ($p) { $p -split ';' | Out-File $outFile -Encoding utf8; Write-Host ('备份已保存至 ' + $outFile) -ForegroundColor Cyan } else { Write-Host '错误：无法获取 PATH 变量。' -ForegroundColor Red }"
pause
goto menu

:end
exit
