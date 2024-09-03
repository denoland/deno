try {
  Deno.removeSync("./lock_write_fetch.json");
} catch {
  // pass
}

const fetchProc = await new Deno.Command(Deno.execPath(), {
  stdout: "null",
  stderr: "null",
  args: [
    "cache",
    "--reload",
    "--lock=lock_write_fetch.json",
    "--cert=tls/RootCA.pem",
    "run/https_import.ts",
  ],
}).output();

console.log(`fetch code: ${fetchProc.code}`);

const fetchCheckProc = await new Deno.Command(Deno.execPath(), {
  stdout: "null",
  stderr: "null",
  args: [
    "cache",
    "--lock=lock_write_fetch.json",
    "--cert=tls/RootCA.pem",
    "run/https_import.ts",
  ],
}).output();

console.log(`fetch check code: ${fetchCheckProc.code}`);

Deno.removeSync("./lock_write_fetch.json");

const runProc = await new Deno.Command(Deno.execPath(), {
  stdout: "null",
  stderr: "null",
  args: [
    "run",
    "--lock=lock_write_fetch.json",
    "--allow-read",
    "--cert=tls/RootCA.pem",
    "run/https_import.ts",
  ],
}).output();

console.log(`run code: ${runProc.code}`);

await Deno.stat("./lock_write_fetch.json");
Deno.removeSync("./lock_write_fetch.json");
