// This test is Linux/Darwin only
if (Deno.build.os !== "linux" && Deno.build.os !== "darwin") {
  console.log("hi\n\n0");
  Deno.exit(123);
}

const cmd = new Deno.Command("/usr/bin/env", {
  args: [
    "bash",
    "-c",
    [Deno.execPath(), "run", "--allow-read", "reader.ts", '<(echo "hi")'].join(
      " ",
    ),
  ],
}).spawn();

console.log((await cmd.status).code);
Deno.exit(123);
