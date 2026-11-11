@echo off
setlocal EnableExtensions
set "CARGO_ENCODED_RUSTFLAGS="
set "RUSTFLAGS=-C target-feature=-crt-static"
cd /d "%~dp0"
set "MODEL_NAME=birefnetv2-img.onnx"
set "TEMP_MODEL=0"
set "MODEL_PATH="

echo ========================================================
echo Building NOT INTEGRATED version (AI Models Manager)...
echo ========================================================
cargo build --release
if errorlevel 1 goto :error
copy /Y target\release\rustbackremover.exe rustbackremover_notintegrated.exe >nul

echo.
if exist "%MODEL_NAME%" goto :build_integrated

echo Looking for %MODEL_NAME% in AI Models Manager...
for /f "tokens=2,*" %%A in ('reg query "HKLM\SOFTWARE\Gallery Inc\AI-ModelsManager\models" /v "%MODEL_NAME%" 2^>nul ^| find /I "%MODEL_NAME%"') do set "MODEL_PATH=%%B"

if not defined MODEL_PATH goto :skip_integrated
if not exist "%MODEL_PATH%" goto :skip_integrated

copy /Y "%MODEL_PATH%" "%MODEL_NAME%" >nul
if errorlevel 1 goto :skip_integrated
set "TEMP_MODEL=1"
echo Model copied temporarily from AI Models Manager.

:build_integrated
echo ========================================================
echo Building INTEGRATED version (Embedded Model)...
echo ========================================================
cargo build --release --features integrated
if errorlevel 1 goto :error
copy /Y target\release\rustbackremover.exe rustbackremover_integrated.exe >nul
if "%TEMP_MODEL%"=="1" del /Q "%MODEL_NAME%" >nul 2>&1
goto :done

:skip_integrated
echo ========================================================
echo INTEGRATED build skipped.
echo Could not find %MODEL_NAME% locally or in AI Models Manager.
echo ========================================================
if "%TEMP_MODEL%"=="1" del /Q "%MODEL_NAME%" >nul 2>&1
goto :done

:done
echo.
echo ========================================================
echo Build complete.
echo   rustbackremover_notintegrated.exe = uses AI Models Manager
if exist "rustbackremover_integrated.exe" echo   rustbackremover_integrated.exe    = model embedded

echo.
echo IMPORTANT after upgrading from the old build:
echo Reinstall the Explorer Actions menu once, either from the GUI button
echo or with the executable you want Explorer to use:
echo   rustbackremover_notintegrated.exe --add
echo   rustbackremover_integrated.exe --add

echo To remove it later:
echo   rustbackremover_notintegrated.exe --remove-context-menu

echo ========================================================
pause
exit /b 0

:error
if "%TEMP_MODEL%"=="1" del /Q "%MODEL_NAME%" >nul 2>&1
echo.
echo Build failed.
pause
exit /b 1
