try {
  Deno.removeSync("./lock_write_fetch.json");
} catch {}

const fetchProc = Deno.run({
  stdout: "null",
  stderr: "null",
  cmd: [
    Deno.execPath(),
    "cache",
    "--reload",
    "--lock=lock_write_fetch.json",
    "--lock-write",
    "https_import.ts",
  ],
});

const fetchCode = (await fetchProc.status()).code;
console.log(`fetch code: ${fetchCode}`);

const fetchCheckProc = Deno.run({
  stdout: "null",
  stderr: "null",
  cmd: [
    Deno.execPath(),
    "cache",
    "--lock=lock_write_fetch.json",
    "https_import.ts",
  ],
});

const fetchCheckProcCode = (await fetchCheckProc.status()).code;
console.log(`fetch check code: ${fetchCheckProcCode}`);

Deno.removeSync("./lock_write_fetch.json");

const runProc = Deno.run({
  stdout: "null",
  stderr: "null",
  cmd: [
    Deno.execPath(),
    "run",
    "--lock=lock_write_fetch.json",
    "--lock-write",
    "--allow-read",
    "file_exists.ts",
    "lock_write_fetch.json",
  ],
});

const runCode = (await runProc.status()).code;
console.log(`run code: ${runCode}`);

Deno.removeSync("./lock_write_fetch.json");
