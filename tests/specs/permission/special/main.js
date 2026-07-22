// Opening certain files requires --allow-all permission. This file is executed with
// --allow-read only.

const testCases = [
  // Allowed, safe
  [["darwin", "linux"], null, "/dev/null"],
  [["darwin", "linux"], null, "/etc/passwd"],
  [["windows"], null, "\\\\.\\nul"],
  // Allowed with --allow-read, system monitoring files
  [["linux"], null, "/proc/pressure/memory"],
  [["linux"], null, "/proc/pressure/cpu"],
  [["linux"], null, "/proc/pressure/io"],
  // Denied, requires `--allow-all`
  [["darwin", "linux"], /NotCapable/, "/dev/ptmx"],
  [["linux"], /NotCapable/, "/proc/self/environ"],
  [["linux"], /NotCapable/, "/proc/self/mem"],
  [["windows"], /NotCapable/, "\\\\.\\PhysicalDrive0"],
];

// No-follow file ops (stat/lstat/readDir/realPath/readLink) used to skip the
// /proc, /dev, /sys guard entirely (GHSA-m87g-7g5m-wpx4). They should now
// require --allow-all for the same paths content reads do.
const noFollowCases = [
  // stat follows the /proc/self/root symlink — without the guard this leaks
  // metadata about arbitrary filesystem paths with only --allow-read=/proc/self.
  [["linux"], "stat", /NotCapable/, "/proc/self/root/etc/shadow"],
  [["linux"], "lstat", /NotCapable/, "/proc/self/environ"],
  [["linux"], "readDir", /NotCapable/, "/proc/self/root"],
  [["linux"], "realPath", /NotCapable/, "/proc/self/environ"],
  [["darwin", "linux"], "stat", /NotCapable/, "/dev/ptmx"],
];

const os = Deno.build.os;
let failed = false;
let ran = false;

function runOp(op, file) {
  switch (op) {
    case "open":
      Deno.readTextFileSync(file);
      return;
    case "stat":
      Deno.statSync(file);
      return;
    case "lstat":
      Deno.lstatSync(file);
      return;
    case "readDir":
      for (const _ of Deno.readDirSync(file)) {
        // drain
      }
      return;
    case "realPath":
      Deno.realPathSync(file);
      return;
    default:
      throw new Error(`unknown op ${op}`);
  }
}

for (const [oses, error, file] of testCases) {
  if (oses.indexOf(os) === -1) {
    console.log(`Skipping test for ${file} on ${os}`);
    continue;
  }
  ran = true;
  try {
    console.log(`Opening ${file}...`);
    runOp("open", file);
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

for (const [oses, op, error, file] of noFollowCases) {
  if (oses.indexOf(os) === -1) {
    console.log(`Skipping ${op} test for ${file} on ${os}`);
    continue;
  }
  ran = true;
  try {
    console.log(`${op} on ${file}...`);
    runOp(op, file);
    console.log(`*** ${op} shouldn't have succeeded: ${file}`);
    failed = true;
  } catch (e) {
    if (String(e).match(error)) {
      console.log(`Got an error (expected) for ${op} ${file}: ${e}`);
    } else {
      console.log(`*** Got an unexpected error for ${op} ${file}: ${e}`);
      failed = true;
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
