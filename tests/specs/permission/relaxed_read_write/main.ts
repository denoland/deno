// Relaxed default profile (gate on, no flags): read and write are confined to
// the cwd and the OS temp directory, and the .git directory inside the cwd is
// deny-listed for write.

// read: a file inside the cwd is readable (write to the cwd is allowed too)
Deno.writeTextFileSync("./inside.txt", "data");
Deno.readTextFileSync("./inside.txt");
console.log("read cwd: ok");

// read/write: a nested subdirectory of the cwd is covered by the cwd grant
Deno.mkdirSync("./nested/deep", { recursive: true });
Deno.writeTextFileSync("./nested/deep/file.txt", "data");
Deno.readTextFileSync("./nested/deep/file.txt");
console.log("read/write nested cwd: ok");

// read: a file inside the OS temp directory is readable
const tempFile = Deno.makeTempFileSync();
Deno.readTextFileSync(tempFile);
console.log("read temp: ok");

// read: a file outside the cwd and the temp directory (the deno executable) is
// denied naming --allow-read
try {
  Deno.readFileSync(Deno.execPath());
  console.log("read outside: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-read") ? "--allow-read" : "?";
  console.log(`read outside: ${err.name} ${named}`);
}

// write: a file inside the cwd is writable
Deno.writeTextFileSync("./writable.txt", "data");
console.log("write cwd: ok");

// write: the .git directory inside the cwd is denied even though the rest of
// the cwd is writable (blocks the .git/hooks/pre-commit persistence vector)
try {
  Deno.writeTextFileSync("./.git/hooks/pre-commit", "#!/bin/sh\n");
  console.log("write .git: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-write") ? "--allow-write" : "?";
  console.log(`write .git: ${err.name} ${named}`);
}

// write: a direct .git/config write is denied by the same lexical .git deny
try {
  Deno.writeTextFileSync("./.git/config", "[core]\n");
  console.log("write .git config: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-write") ? "--allow-write" : "?";
  console.log(`write .git config: ${err.name} ${named}`);
}

// write: a file elsewhere in the cwd still succeeds
Deno.writeTextFileSync("./elsewhere.txt", "data");
console.log("write elsewhere: ok");
