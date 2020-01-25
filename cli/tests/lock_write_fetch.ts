try {
  Deno.removeSync("./lock_write_fetch.json");
} catch {}

const fetchProc = Deno.run({
  stdout: "null",
  stderr: "null",
  args: [
    Deno.execPath(),
    "--reload",
    "--lock=lock_write_fetch.json",
    "--lock-write",
    "https_import.ts"
  ]
});

const fetchCode = (await fetchProc.status()).code;
console.log(`fetch code: ${fetchCode}`);

const runProc = Deno.run({
  stdout: "null",
  stderr: "null",
  args: [Deno.execPath(), "--lock=lock_write_fetch.json", "https_import.ts"]
});

const runCode = (await runProc.status()).code;
console.log(`run code: ${runCode}`);

Deno.removeSync("./lock_write_fetch.json");
