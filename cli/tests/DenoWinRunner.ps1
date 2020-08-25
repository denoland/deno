$Source = [IO.File]::ReadAllText("$PSScriptRoot\DenoWinRunner.cs")
$denoExePath = $args[0]
$scriptPath = $args[1]
$constraints = $args[2]
$timeout = 5000;
Add-Type -TypeDefinition $Source -Language CSharp
Write-Output("Running Deno script: " + $args[1])
$code = [DenoWinRunner]::RunDenoScript($denoExePath, $scriptPath, $constraints, $timeout)
Write-Output("Deno.exe or the test wrapper has exited with code: $code")
exit $code
