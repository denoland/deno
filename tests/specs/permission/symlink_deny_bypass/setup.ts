Deno.mkdirSync("allowed", { recursive: true });
Deno.mkdirSync("denied", { recursive: true });
Deno.writeTextFileSync("denied/secret.txt", "SECRET DATA");
// symlink in allowed/ pointing to the denied file
Deno.symlinkSync(`${Deno.cwd()}/denied/secret.txt`, "allowed/link");
console.log("setup done");
