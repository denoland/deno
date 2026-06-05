// Regression test for https://github.com/denoland/deno/issues/34919
//
// Running a script that the kernel can't exec directly (a shell script
// without a shebang line) must fall back to running it via `/bin/sh`, matching
// the POSIX `execvp` / Node.js behavior. On Linux, Rust's `posix_spawnp` does
// not perform this fallback (unlike macOS libc), so Deno does it explicitly.
import { execFile, execFileSync, spawn, spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";

// A shell script with no shebang line.
const script = "./script.sh";
writeFileSync(script, 'echo "$1 $1";\n', { mode: 0o755 });

// 1. execFileSync (the originally reported case)
const out = execFileSync(script, ["hello"], { encoding: "utf8" });
console.log("execFileSync:", out.trim());

// 2. spawnSync
const res = spawnSync(script, ["hi"], { encoding: "utf8" });
console.log("spawnSync:", res.stdout.trim(), "status:", res.status);

// 3. Deno.Command (sync)
const denoOut = new Deno.Command(script, { args: ["deno"] }).outputSync();
console.log(
  "Deno.Command sync:",
  new TextDecoder().decode(denoOut.stdout).trim(),
);

// 4. async execFile
await new Promise<void>((resolve, reject) => {
  execFile(script, ["async"], { encoding: "utf8" }, (err, stdout) => {
    if (err) return reject(err);
    console.log("execFile:", stdout.trim());
    resolve();
  });
});

// 5. async spawn
await new Promise<void>((resolve, reject) => {
  const child = spawn(script, ["spawn"], { stdio: ["ignore", "pipe", "pipe"] });
  let buf = "";
  child.stdout.on("data", (d) => buf += d);
  child.on("error", reject);
  child.on("close", () => {
    console.log("spawn:", buf.trim());
    resolve();
  });
});
