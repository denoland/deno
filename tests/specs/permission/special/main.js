// Opening certain files requires --allow-all permission. This file is executed with
// --allow-read only.

const testCases = [
  // Allowed, safe
  [["darwin", "linux"], null, "/dev/null"],
  [["darwin", "linux"], null, "/etc/passwd"],
  [["windows"], null, "\\\\.\\nul"],
  // Denied, requires `--allow-all`
  [["darwin", "linux"], /NotCapable/, "/dev/ptmx"],
  [["linux"], /NotCapable/, "/proc/self/environ"],
  [["linux"], /NotCapable/, "/proc/self/mem"],
  [["windows"], /NotCapable/, "\\\\.\\PhysicalDrive0"],
];

const os = Deno.build.os;
let failed = false;
let ran = false;

for (const [oses, error, file] of testCases) {
  if (oses.indexOf(os) === -1) {
    console.log(`Skipping test for ${file} on ${os}`);
    continue;
  }
  ran = true;
  try {
    console.log(`Opening ${file}...`);
    Deno.readTextFileSync(file);
    if (error === null) {
      console.log("Succeeded, as expected.");
    } else {
      console.log(`*** Shouldn't have succeeded: ${file}`);
      failed = true;
    }
  } catch (e) {
    if (error === null) {
      console.log(`*** Shouldn't have failed: ${file}: ${e}`);
      failed = true;
    } else {
      if (String(e).match(error)) {
        console.log(`Got an error (expected) for ${file}: ${e}`);
      } else {
        console.log(`*** Got an unexpected error for ${file}: ${e}`);
        failed = true;
      }
    }
  }
}

if (!ran) {
  console.log(`Uh-oh: didn't run any tests for ${Deno.build.os}.`);
  failed = true;
}
if (failed) {
  console.log("One or more tests failed");
}
Deno.exit(failed ? 321 : 123);
