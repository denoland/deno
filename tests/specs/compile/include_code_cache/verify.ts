import { basename, join } from "node:path";
import { tmpdir } from "node:os";

const executable = join(
  Deno.cwd(),
  Deno.build.os === "windows" ? "embedded_cache.exe" : "embedded_cache",
);
const externalCache = join(
  tmpdir(),
  `deno-compile-${basename(executable)}.cache`,
);
try {
  await Deno.remove(externalCache);
} catch (error) {
  if (!(error instanceof Deno.errors.NotFound)) {
    throw error;
  }
}

const output = new Deno.Command(executable).outputSync();
const stdout = new TextDecoder().decode(output.stdout);
const stderr = new TextDecoder().decode(output.stderr);

if (!output.success) {
  throw new Error(`compiled executable failed: ${stderr}`);
}
if (!/Loaded \d+ embedded code cache entries/.test(stderr)) {
  throw new Error(`embedded code cache was not loaded:\n${stderr}`);
}
if (stderr.includes("Serialized ")) {
  throw new Error(
    `first run unexpectedly serialized an external cache:\n${stderr}`,
  );
}
if (!stdout.includes("[Function: join]")) {
  throw new Error(`compiled executable produced unexpected output:\n${stdout}`);
}
if (Deno.readTextFileSync("main-evaluated.txt") !== "evaluated") {
  throw new Error("compiled executable did not evaluate the main module");
}

try {
  Deno.statSync(externalCache);
  throw new Error(`unexpected external code cache at ${externalCache}`);
} catch (error) {
  if (!(error instanceof Deno.errors.NotFound)) {
    throw error;
  }
}

console.log("embedded code cache verified");
