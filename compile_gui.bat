@echo off
setlocal

set AHK_COMPILER="C:\Program Files\AutoHotkey\Compiler\Ahk2Exe.exe"
set SOURCE_SCRIPT=giagui.ahk
set OUTPUT_EXE=giagui.exe
set ICON_FILE=icons\gia.ico

echo Compiling %SOURCE_SCRIPT% to %OUTPUT_EXE%...

%AHK_COMPILER% /in %SOURCE_SCRIPT% /out %OUTPUT_EXE% /icon %ICON_FILE%

if %ERRORLEVEL% EQU 0 (
    echo.
    echo Compilation successful!
    echo Output: %OUTPUT_EXE%
) else (
    echo.
    echo Compilation failed with error code %ERRORLEVEL%
    exit /b %ERRORLEVEL%
)

endlocal
