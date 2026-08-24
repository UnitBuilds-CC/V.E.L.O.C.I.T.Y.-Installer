@echo off
REM Fake InnoSetup installer - logs received arguments
set LOGFILE=%~dp0innosetup_log.txt
echo FAKE_INNOSETUP_INSTALLER > "%LOGFILE%"
echo Args: %* >> "%LOGFILE%"
echo WorkingDir: %CD% >> "%LOGFILE%"
date /t >> "%LOGFILE%"
time /t >> "%LOGFILE%"
REM Exit with 0 (success)
exit /b 0
