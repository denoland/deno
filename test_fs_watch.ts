const watcher = Deno.watch(".", { recursive: true });

console.log("starting watcher");
setTimeout(() => {
  watcher.close();
  console.log("ending!");
}, 30000);

for await (const event of watcher) {
  console.log("got event!", event);
}
