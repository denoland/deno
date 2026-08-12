// Regression test for #36034: when the snapshot cannot be written, no
// `.heapsnapshot` (in particular, no 0-byte one) may be left behind, and the
// failure must be reported.
//
// The failure is injected by making the child's working directory read-only at
// the OS level. Deno's own write permission is still granted, so the op
// installs the callback and the failure only happens later, inside the
// near-heap-limit callback, when it tries to create the file.
//
// chmod does nothing on Windows and is bypassed by root, so skip in both cases.
const dir = Deno.makeTempDirSync();

function dirIsWritable() {
  try {
    Deno.writeTextFileSync(`${dir}/probe`, "");
    Deno.removeSync(`${dir}/probe`);
    return true;
  } catch {
    return false;
  }
}

if (Deno.build.os !== "windows") {
  Deno.chmodSync(dir, 0o555);
}

if (Deno.build.os === "windows" || dirIsWritable()) {
  // Can't make the directory read-only here, so there is nothing to assert.
  if (Deno.build.os !== "windows") {
    Deno.chmodSync(dir, 0o755);
  }
  Deno.removeSync(dir, { recursive: true });
  console.log("write-failure-no-file");
} else {
  const cmd = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--allow-write=.",
      "--v8-flags=--max-old-space-size=20",
      `${Deno.cwd()}/oom.mjs`,
    ],
    cwd: dir,
    stdout: "null",
    stderr: "piped",
  });
  const { stderr } = await cmd.output();
  const text = new TextDecoder().decode(stderr);

  Deno.chmodSync(dir, 0o755);

  if (!text.includes("Failed to write heap snapshot to ")) {
    console.error("child did not report a heap snapshot write failure");
    console.error(text);
    Deno.exit(1);
  }
  for (const entry of Deno.readDirSync(dir)) {
    console.error(`unexpected leftover file: ${entry.name}`);
    Deno.exit(1);
  }
  Deno.removeSync(dir, { recursive: true });
  console.log("write-failure-no-file");
}
