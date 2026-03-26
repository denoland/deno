import process from "node:process";

// In a compiled binary, process.argv[1] should be the binary path
// (same as Deno.execPath()), not an internal temp extraction path
// like "deno-compile-<name>/path/to/module.ts".
const argv1 = process.argv[1];
const execPath = Deno.execPath();

// argv[1] should equal the binary path
if (argv1 !== execPath) {
  console.error(
    `FAIL: process.argv[1] (${argv1}) !== Deno.execPath() (${execPath})`,
  );
  process.exit(1);
}

// User args should start at argv[2]
const userArgs = process.argv.slice(2);
console.log(`argv1_ok`);
console.log(`user_args: ${JSON.stringify(userArgs)}`);
