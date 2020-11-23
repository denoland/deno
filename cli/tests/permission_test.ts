// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const name = Deno.args[0];
// deno-lint-ignore no-explicit-any
const test: { [key: string]: (...args: any[]) => void | Promise<void> } = {
  readRequired(): Promise<void> {
    Deno.readFileSync("README.md");
    return Promise.resolve();
  },
  writeRequired(): void {
    Deno.makeTempDirSync();
  },
  envRequired(): void {
    Deno.env.get("home");
  },
  netRequired(): void {
    Deno.listen({ transport: "tcp", port: 4541 });
  },
  runRequired(): void {
    const p = Deno.run({
      cmd: Deno.build.os === "windows"
        ? ["cmd.exe", "/c", "echo hello"]
        : ["printf", "hello"],
    });
    p.close();
  },
};

if (!test[name]) {
  console.log("Unknown test:", name);
  Deno.exit(1);
}

test[name]();
