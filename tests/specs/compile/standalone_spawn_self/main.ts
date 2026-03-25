import { spawn } from "node:child_process";
import process from "node:process";

if (process.env.SPAWNED_CHILD === "1") {
  // Child process: verify no Deno flags were injected into argv.
  // Before the fix, child_process.spawn would translate Node args
  // into Deno args (run -A --unstable-node-globals etc.) when the
  // spawn target was a compiled binary.
  const appArgs = process.argv.slice(2);
  const denoFlags = appArgs.filter(
    (a) =>
      a === "run" ||
      a === "-A" ||
      a.startsWith("--unstable-") ||
      a.startsWith("--v8-flags="),
  );
  if (denoFlags.length > 0) {
    console.log("FAIL: unexpected Deno flags in argv: " + denoFlags.join(", "));
    process.exit(1);
  }
  console.log("ok");
} else {
  // Parent process: re-spawn self with the script path
  // (mimics the pattern used by Node CLIs that relaunch themselves)
  const script = process.argv[1];
  const child = spawn(process.execPath, [script], {
    stdio: ["pipe", "pipe", "pipe"],
    env: { ...process.env, SPAWNED_CHILD: "1" },
  });

  let stdout = "";
  child.stdout!.on("data", (data) => {
    stdout += data.toString();
  });

  let stderr = "";
  child.stderr!.on("data", (data) => {
    stderr += data.toString();
  });

  child.on("close", (code) => {
    if (stderr) {
      process.stderr.write(stderr);
    }
    process.stdout.write(stdout);
    process.exit(code ?? 1);
  });
}
