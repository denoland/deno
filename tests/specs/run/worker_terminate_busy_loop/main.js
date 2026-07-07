// Copyright 2018-2026 the Deno authors. MIT license.

const command = new Deno.Command(Deno.execPath(), {
  args: [
    "run",
    "-A",
    "--quiet",
    new URL("./child.js", import.meta.url).pathname,
  ],
  stdout: "piped",
  stderr: "piped",
});
const child = command.spawn();
const statusPromise = child.status;

let timedOut = false;
const timeout = setTimeout(() => {
  timedOut = true;
  child.kill("SIGKILL");
}, 5000);

const stdoutReader = child.stdout.getReader();
const decoder = new TextDecoder();
let text = "";
while (!text.includes("\n")) {
  const { done, value } = await stdoutReader.read();
  if (done) break;
  text += decoder.decode(value, { stream: true });
}
text += decoder.decode();
if (text !== "terminating\n") {
  try {
    child.kill("SIGKILL");
  } catch {
    // The child process has already exited.
  }
  await statusPromise;
  clearTimeout(timeout);
  const stderr = await new Response(child.stderr).text();
  if (timedOut) {
    throw new Error(`Child timed out\n${stderr}`);
  }
  throw new Error(`Unexpected child output: ${text}\n${stderr}`);
}

const status = await statusPromise;
clearTimeout(timeout);

if (!status.success) {
  const stderr = await new Response(child.stderr).text();
  if (timedOut) {
    throw new Error(`Child timed out\n${stderr}`);
  }
  throw new Error(stderr);
}

console.log("terminated");
