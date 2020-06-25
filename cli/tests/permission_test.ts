// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const name = Deno.args[0];
const test: { [key: string]: Function } = {
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
    Deno.run({
      cmd: [
        "python",
        "-c",
        "import sys; sys.stdout.write('hello'); sys.stdout.flush()",
      ],
    });
  },
};

if (!test[name]) {
  console.log("Unknown test:", name);
  Deno.exit(1);
}

test[name]();
