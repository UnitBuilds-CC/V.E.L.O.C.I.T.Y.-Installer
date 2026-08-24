@echo off
REM Fake installer that simulates a reboot-required exit (exit code 3010)
set LOGFILE=%~dp0reboot_log.txt
echo FAKE_REBOOT_REQUIRED_INSTALLER > "%LOGFILE%"
echo Args: %* >> "%LOGFILE%"
date /t >> "%LOGFILE%"
time /t >> "%LOGFILE%"
REM Exit with 3010 (reboot required - acceptable exit code)
exit /b 3010
