try {
  Deno.removeSync("./lock_write_fetch.json");
} catch {
  // pass
}

const fetchProc = await Deno.spawn(Deno.execPath(), {
  stdout: "null",
  stderr: "null",
  args: [
    "cache",
    "--reload",
    "--lock=lock_write_fetch.json",
    "--lock-write",
    "--cert=tls/RootCA.pem",
    "https_import.ts",
  ],
});

console.log(`fetch code: ${fetchProc.status.code}`);

const fetchCheckProc = await Deno.spawn(Deno.execPath(), {
  stdout: "null",
  stderr: "null",
  args: [
    "cache",
    "--lock=lock_write_fetch.json",
    "--cert=tls/RootCA.pem",
    "https_import.ts",
  ],
});

console.log(`fetch check code: ${fetchCheckProc.status.code}`);

Deno.removeSync("./lock_write_fetch.json");

const runProc = await Deno.spawn(Deno.execPath(), {
  stdout: "null",
  stderr: "null",
  args: [
    "run",
    "--lock=lock_write_fetch.json",
    "--lock-write",
    "--allow-read",
    "file_exists.ts",
    "lock_write_fetch.json",
  ],
});

console.log(`run code: ${runProc.status.code}`);

Deno.removeSync("./lock_write_fetch.json");
