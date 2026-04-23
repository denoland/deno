// Child process that reports back to parent
// If the args were double-translated, this process would fail to start
// or would have incorrect behavior

process.send({
  result: "ok",
  // Include some process info to verify the child started correctly
  pid: process.pid,
  argv: process.argv.slice(2), // script args
});
