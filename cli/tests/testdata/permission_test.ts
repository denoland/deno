// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const name = Deno.args[0];
// deno-lint-ignore no-explicit-any
const test: { [key: string]: (...args: any[]) => void | Promise<void> } = {
  readRequired() {
    Deno.readFileSync("hello.txt");
    return Promise.resolve();
  },
  writeRequired() {
    Deno.makeTempDirSync();
  },
  envRequired() {
    Deno.env.get("home");
  },
  netRequired() {
    Deno.listen({ transport: "tcp", port: 4541 });
  },
  async runRequired() {
    await Deno.spawn(Deno.build.os === "windows" ? "cmd.exe" : "printf", {
      args: Deno.build.os === "windows" ? ["/c", "echo hello"] : ["hello"],
    });
  },
};

if (!test[name]) {
  console.log("Unknown test:", name);
  Deno.exit(1);
}

test[name]();
