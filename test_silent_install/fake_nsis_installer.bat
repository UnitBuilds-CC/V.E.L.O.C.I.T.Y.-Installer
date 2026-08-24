@echo off
REM Fake NSIS installer - logs received arguments to prove silent flags were passed
set LOGFILE=%~dp0install_log.txt
echo FAKE_NSIS_INSTALLER > "%LOGFILE%"
echo Args: %* >> "%LOGFILE%"
echo WorkingDir: %CD% >> "%LOGFILE%"
echo ScriptDir: %~dp0 >> "%LOGFILE%"
date /t >> "%LOGFILE%"
time /t >> "%LOGFILE%"
REM Exit with 0 (success)
exit /b 0
