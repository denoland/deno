@echo off
rem Stands in for the jsr binary: prints the arguments it was run with.
echo jsr %*
if defined FAKE_JSR_EXIT_CODE exit /b %FAKE_JSR_EXIT_CODE%
