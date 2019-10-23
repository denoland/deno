// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { args, env, exit, listen, makeTempDirSync, readFileSync, run } = Deno;

const firstCheckFailedMessage = "First check failed";

const name = args[1];
const test = {
  async needsRead(): Promise<void> {
    try {
      readFileSync("package.json");
    } catch (e) {
      console.log(firstCheckFailedMessage);
    }
    readFileSync("package.json");
  },
  needsWrite(): void {
    try {
      makeTempDirSync();
    } catch (e) {
      console.log(firstCheckFailedMessage);
    }
    makeTempDirSync();
  },
  needsEnv(): void {
    try {
      env().home;
    } catch (e) {
      console.log(firstCheckFailedMessage);
    }
    env().home;
  },
  needsNet(): void {
    try {
      listen({ hostname: "127.0.0.1", port: 4540 });
    } catch (e) {
      console.log(firstCheckFailedMessage);
    }
    listen({ hostname: "127.0.0.1", port: 4541 });
  },
  needsRun(): void {
    try {
      run({
        args: [
          "python",
          "-c",
          "import sys; sys.stdout.write('hello'); sys.stdout.flush()"
        ]
      });
    } catch (e) {
      console.log(firstCheckFailedMessage);
    }
    run({
      args: [
        "python",
        "-c",
        "import sys; sys.stdout.write('hello'); sys.stdout.flush()"
      ]
    });
  }
}[name];

if (!test) {
  console.log("Unknown test:", name);
  exit(1);
}

test();
