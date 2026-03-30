// Test that path permissions are case-insensitive on Windows.
// A deny-read rule should block access regardless of the path casing.
const dir = Deno.makeTempDirSync();
const file = dir + "\\secret.txt";
Deno.writeTextFileSync(file, "secret");

const lowerDir = dir.toLowerCase();
const lowerFile = lowerDir + "\\secret.txt";

// Write the test script to a separate temp dir (not under the denied path).
const scriptDir = Deno.makeTempDirSync();
const scriptFile = scriptDir + "\\test_read.ts";
Deno.writeTextFileSync(
  scriptFile,
  `try {
  Deno.readTextFileSync(${JSON.stringify(lowerFile)});
  console.log("BYPASSED");
} catch (e) {
  console.log("BLOCKED:" + e.constructor.name);
}
`,
);

// Spawn a subprocess with --deny-read on the original-case path,
// then run the script which reads via the lowered-case path.
const result = new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--allow-read",
    `--deny-read=${dir}`,
    scriptFile,
  ],
}).outputSync();

const stdout = new TextDecoder().decode(result.stdout).trim();
const stderr = new TextDecoder().decode(result.stderr).trim();
if (stdout) {
  console.log(stdout);
} else {
  console.log(stderr || "NO OUTPUT");
}

Deno.removeSync(dir, { recursive: true });
Deno.removeSync(scriptDir, { recursive: true });
