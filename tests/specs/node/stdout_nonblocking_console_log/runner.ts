const child = new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "--quiet",
    "--allow-ffi",
    new URL("child.ts", import.meta.url).pathname,
  ],
  stdout: "piped",
  stderr: "piped",
}).spawn();

await new Promise((resolve) => setTimeout(resolve, 100));

const [status, stdout, stderr] = await Promise.all([
  child.status,
  new Response(child.stdout).arrayBuffer(),
  new Response(child.stderr).text(),
]);

if (!status.success) {
  throw new Error(stderr);
}
if (stdout.byteLength === 0) {
  throw new Error("missing stdout");
}
if (!stderr.includes("ok")) {
  throw new Error("missing stderr marker");
}

console.log("ok");
