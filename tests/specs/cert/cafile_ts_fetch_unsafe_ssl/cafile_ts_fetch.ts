fetch("https://localhost:5545/cert/cafile_ts_fetch.ts.out")
  .then((r) => r.text())
  .then((t) => console.log(t.trimEnd()));
