// Regression test: modules given via --preload (or --require / NODE_OPTIONS)
// must execute under --watch, both on the initial run and again after every
// watcher restart. They were silently skipped because the watch path never
// called execute_preload_modules().

const preload = `${Deno.cwd()}/preload.js`;
const watched = `${Deno.cwd()}/watched.js`;
Deno.writeTextFileSync(preload, 'console.log("preload ran");\n');
Deno.writeTextFileSync(watched, 'console.log("main ran");\n');

const deadline = setTimeout(() => {
  console.error("test timed out waiting for preload output");
  Deno.exit(1);
}, 60_000);
Deno.unrefTimer(deadline);

const child = new Deno.Command(Deno.execPath(), {
  args: ["run", "--watch", "--preload", preload, watched],
  stdout: "piped",
  stderr: "null",
  env: { NO_COLOR: "1" },
}).spawn();

// Count "preload ran" / "main ran" lines on the child's stdout and keep
// draining the stream so the child can't block on a full pipe.
let preloadRuns = 0;
let mainRuns = 0;
let notify = () => {};
(async () => {
  let buffered = "";
  for await (const chunk of child.stdout.pipeThrough(new TextDecoderStream())) {
    buffered += chunk;
    let newlineIndex;
    while ((newlineIndex = buffered.indexOf("\n")) >= 0) {
      const line = buffered.slice(0, newlineIndex);
      buffered = buffered.slice(newlineIndex + 1);
      if (line === "preload ran") preloadRuns++;
      if (line === "main ran") mainRuns++;
      notify();
    }
  }
})();

async function waitFor(predicate) {
  while (!predicate()) {
    await new Promise((resolve) => notify = resolve);
  }
}

// Initial run: the preload must execute before the main module.
await waitFor(() => preloadRuns >= 1 && mainRuns >= 1);
console.log("preload ran on initial run");

// Trigger watcher restarts; re-writing the file guards against a change
// event being missed or debounced away.
const rewrite = setInterval(() => {
  Deno.writeTextFileSync(watched, 'console.log("main ran");\n');
}, 250);

// After a restart the preload must run again.
await waitFor(() => preloadRuns >= 2 && mainRuns >= 2);
clearInterval(rewrite);
console.log("preload ran again after watcher restart");

child.kill("SIGTERM");
await child.status;
