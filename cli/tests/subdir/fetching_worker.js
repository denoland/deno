fetch("http://localhost:4545/cli/tests/subdir/fetching_worker.js")
  .then(r => r.json())
  .then(console.log, console.error);
