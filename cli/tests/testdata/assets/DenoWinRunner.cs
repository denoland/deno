using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

[Flags]
public enum DenoConstraints : int
{
    None = 0,
    NoStdin = 1,
    NoStdout = 2,
    NoStderr = 4
}

public class DenoWinRunner
{
    private const int STD_INPUT_HANDLE = -10;
    private const int STD_OUTPUT_HANDLE = -11;
    private const int STD_ERROR_HANDLE = -12;

    private const int FILE_NOT_FOUND = 2;
    private const int WAIT_TIMEOUT = 258;

    [DllImport("kernel32.dll")]
    private static extern void SetStdHandle(int nStdHandle, IntPtr handle);

    /// <summary>
    /// Runs Deno.exe under the specified constraints
    /// </summary>
    /// <param name="pathToDenoExe">Path to the Deno.exe file. Can be absolute or relative</param>
    /// <param name="pathToTestScript">Path to the script file Deno should run.</param>
    /// <param name="constraints">The constraints to apply to the Deno process</param>
    /// <param name="timeoutMilliseconds">How long to wait for the Deno process to exit</param>
    /// <returns>The deno.exe exit code, or an exit code provided by the test runner</returns>
    public static int RunDenoScript(string pathToDenoExe, string pathToTestScript, DenoConstraints constraints, uint timeoutMilliseconds = 1000)
    {
        try
        {
            if (!File.Exists(pathToDenoExe))
            {
                Console.Error.WriteLine("Cannot find Deno.exe at " + pathToDenoExe);
                return FILE_NOT_FOUND;
            }

            if (!File.Exists(pathToTestScript))
            {
                Console.Error.WriteLine("Cannot find test script at " + pathToTestScript);
                return FILE_NOT_FOUND;
            }

            ProcessStartInfo startInfo = new ProcessStartInfo(pathToDenoExe)
            {
                ErrorDialog = false,
                UseShellExecute = false,
                Arguments = @"run -A " + pathToTestScript,
                RedirectStandardInput = !constraints.HasFlag(DenoConstraints.NoStdin),
                RedirectStandardOutput = !constraints.HasFlag(DenoConstraints.NoStdout),
                RedirectStandardError = !constraints.HasFlag(DenoConstraints.NoStderr)
            };

            startInfo.Environment.Add("RUST_BACKTRACE", "1");

            if (constraints.HasFlag(DenoConstraints.NoStdin))
            {
                SetStdHandle(STD_INPUT_HANDLE, (IntPtr)null);
            }

            if (constraints.HasFlag(DenoConstraints.NoStdout))
            {
                SetStdHandle(STD_OUTPUT_HANDLE, (IntPtr)null);
            }

            if (constraints.HasFlag(DenoConstraints.NoStderr))
            {
                SetStdHandle(STD_ERROR_HANDLE, (IntPtr)null);
            }

            Process process = new Process { StartInfo = startInfo };
            process.Start();

            Task<string> stdErrTask = startInfo.RedirectStandardError ?
                process.StandardError.ReadToEndAsync() : Task.FromResult<string>(null);
            Task<string> stdOutTask = startInfo.RedirectStandardOutput ?
                process.StandardOutput.ReadToEndAsync() : Task.FromResult<string>(null);

            if (!process.WaitForExit((int)timeoutMilliseconds))
            {
                Console.Error.WriteLine("Timed out waiting for Deno process to exit");
                try
                {
                    process.Kill();
                }
                catch
                {
                    // Kill might fail, either because the process already exited or due to some other error
                    Console.Error.WriteLine("Failure killing the Deno process - possible Zombie Deno.exe process");
                }
                return WAIT_TIMEOUT;
            }

            // If the Deno process wrote to STDERR - append it to our STDERR
            if (!constraints.HasFlag(DenoConstraints.NoStderr))
            {
                string error = stdErrTask.Result;
                if (!string.IsNullOrWhiteSpace(error))
                {
                    Console.Error.WriteLine(error);
                }
            }

            return process.ExitCode;

        }
        catch (Win32Exception ex)
        {
            Console.Error.WriteLine("Win32Exception: code = " + ex.ErrorCode + ", message: " + ex.Message);
            return ex.NativeErrorCode;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine("Exception: message: " + ex.Message);
            return -1;
        }
    }
}
