// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { args, listen, env, exit, makeTempDirSync, readFileSync, run } = Deno;

const name = args[1];
const test = {
  needsRead: async () => {
    try {
      readFileSync("package.json");
    } catch (e) {
      console.log("First check failed");
    }
    readFileSync("package.json");
  },
  needsWrite: () => {
    try {
      makeTempDirSync();
    } catch (e) {
      console.log("First check failed");
    }
    makeTempDirSync();
  },
  needsEnv: () => {
    try {
      env().home;
    } catch (e) {
      console.log("First check failed");
    }
    env().home;
  },
  needsNet: () => {
    try {
      listen("tcp", "127.0.0.1:4540");
    } catch (e) {
      console.log("First check failed");
    }
    listen("tcp", "127.0.0.1:4541");
  },
  needsRun: async () => {
    try {
      const process = run({
        args: [
          "python",
          "-c",
          "import sys; sys.stdout.write('hello'); sys.stdout.flush()"
        ]
      });
    } catch (e) {
      console.log("First check failed");
    }
    const process = run({
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
