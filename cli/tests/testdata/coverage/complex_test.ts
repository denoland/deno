import { complex } from "./complex.ts";

Deno.test("complex", function () {
  complex("foo", "bar", "baz");
});

Deno.test("sub process with stdin", async () => {
  // ensure launching deno run with stdin doesn't affect coverage
  const code = "console.log('5')";
  const p = await Deno.run({
    cmd: [Deno.execPath(), "run", "-"],
    stdin: "piped",
    stdout: "piped",
  });
  const encoder = new TextEncoder();
  await p.stdin.write(encoder.encode(code));
  await p.stdin.close();
  const output = new TextDecoder().decode(await p.output());
  p.close();
  if (output.trim() !== "5") {
    throw new Error("Failed");
  }
});

Deno.test("sub process with deno eval", async () => {
  // ensure launching deno eval doesn't affect coverage
  const code = "console.log('5')";
  const p = await Deno.run({
    cmd: [Deno.execPath(), "eval", code],
    stdout: "piped",
  });
  const output = new TextDecoder().decode(await p.output());
  p.close();
  if (output.trim() !== "5") {
    throw new Error("Failed");
  }
});
