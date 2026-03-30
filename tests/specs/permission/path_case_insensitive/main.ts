// Test that path permissions are case-insensitive on Windows.
// A deny-read rule should block access regardless of the path casing.
const dir = Deno.makeTempDirSync();
const file = dir + "\\secret.txt";
Deno.writeTextFileSync(file, "secret");

const lowerDir = dir.toLowerCase();

// Spawn a subprocess with --deny-read on the original-case path,
// then try reading via the lowered-case path.
const result = new Deno.Command(Deno.execPath(), {
  args: [
    "eval",
    "--allow-read",
    `--deny-read=${dir}`,
    `try { Deno.readTextFileSync("${
      lowerDir.replace(/\\/g, "\\\\")
    }\\\\secret.txt"); console.log("BYPASSED"); } catch (e) { console.log("BLOCKED:" + e.constructor.name); }`,
  ],
}).outputSync();

const output = new TextDecoder().decode(result.stdout).trim();
console.log(output);

Deno.removeSync(dir, { recursive: true });
