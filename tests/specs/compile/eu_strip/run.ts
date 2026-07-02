// Regression test for https://github.com/denoland/deno/issues/35633
//
// `flatpak-builder` runs `eu-strip` (elfutils) over compiled binaries. The
// standalone binary embeds its payload via a note appended past the original
// EOF, together with a relocated program header table. `eu-strip` lays the
// stripped output out itself and zero-fills the file gap the program header
// table lives in unless an allocated section covers it, producing an all-zero
// program header table that segfaults on exec. This runs `eu-strip` over the
// compiled binary and asserts it still executes.
const bin = "./main";

let strip;
try {
  strip = await new Deno.Command("eu-strip", {
    args: [bin],
    stdout: "null",
    stderr: "piped",
  }).output();
} catch (err) {
  if (err instanceof Deno.errors.NotFound) {
    // elfutils isn't installed on this machine; nothing to exercise.
    console.log("OK (eu-strip unavailable)");
    Deno.exit(0);
  }
  throw err;
}

if (!strip.success) {
  // eu-strip refused the file (e.g. already stripped); treat as a skip.
  console.log("OK (eu-strip refused the file)");
  Deno.exit(0);
}

const res = await new Deno.Command(bin, { stdout: "piped", stderr: "piped" })
  .output();
if (!res.success) {
  console.error(new TextDecoder().decode(res.stderr));
  console.error(`stripped binary exited with ${res.code}`);
  Deno.exit(1);
}

const out = new TextDecoder().decode(res.stdout).trim();
if (out !== "hello from compiled binary") {
  console.error(`unexpected output from stripped binary: ${JSON.stringify(out)}`);
  Deno.exit(1);
}

console.log("OK");
