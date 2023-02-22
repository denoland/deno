// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

if (Deno.build.os !== "linux") {
  throw new Error("SO_REUSEPORT is only supported on Linux");
}

const executable = Deno.execPath();
const path = new URL("./deno_http_flash_ops.js", import.meta.url).pathname;
// single flash instance runs on ~1.8 cores
const cpus = navigator.hardwareConcurrency / 2;
const processes = new Array(cpus);
for (let i = 0; i < cpus; i++) {
  const proc = Deno.run({
    cmd: [executable, "run", "-A", "--unstable", path, Deno.args[0]],
  });
  processes.push(proc.status());
}
await Promise.all(processes);
