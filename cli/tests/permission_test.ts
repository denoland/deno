// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { args, listen, env, exit, makeTempDirSync, readFileSync, run } = Deno;

const name = args[0];
const test: { [key: string]: Function } = {
  readRequired(): Promise<void> {
    readFileSync("README.md");
    return Promise.resolve();
  },
  writeRequired(): void {
    makeTempDirSync();
  },
  envRequired(): void {
    env().home;
  },
  netRequired(): void {
    listen({ transport: "tcp", port: 4541 });
  },
  runRequired(): void {
    run({
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
  exit(1);
}

test[name]();
