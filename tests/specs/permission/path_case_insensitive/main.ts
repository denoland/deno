// Test that path permissions are case-insensitive on Windows.
// A deny-read rule should block access regardless of the path casing.
const dir = Deno.makeTempDirSync();
const file = dir + "\\secret.txt";
Deno.writeTextFileSync(file, "secret");

const lowerDir = dir.toLowerCase();
const lowerFile = lowerDir + "\\secret.txt";

// Spawn a subprocess with --deny-read on the original-case path,
// then try reading via the lowered-case path.
const code = `
try {
  Deno.readTextFileSync(${JSON.stringify(lowerFile)});
  console.log("BYPASSED");
} catch (e) {
  console.log("BLOCKED:" + e.constructor.name);
}
`;
const result = new Deno.Command(Deno.execPath(), {
  args: [
    "eval",
    "--allow-read",
    `--deny-read=${dir}`,
    code,
  ],
}).outputSync();

const stdout = new TextDecoder().decode(result.stdout).trim();
const stderr = new TextDecoder().decode(result.stderr).trim();
if (stdout) {
  console.log(stdout);
} else {
  // If no stdout, print stderr for debugging
  console.log(stderr || "NO OUTPUT");
}

Deno.removeSync(dir, { recursive: true });
