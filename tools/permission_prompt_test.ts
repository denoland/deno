// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const {
  args,
  listen,
  env,
  exit,
  makeTempDirSync,
  readFileSync,
  run,
  openPlugin
} = Deno;

const firstCheckFailedMessage = "First check failed";

const name = args[1];
const test = {
  needsRead: async (): Promise<void> => {
    try {
      readFileSync("package.json");
    } catch (e) {
      if (e.kind === Deno.ErrorKind.PermissionDenied) {
        console.log(firstCheckFailedMessage);
      }
    }
    readFileSync("package.json");
  },
  needsWrite: (): void => {
    try {
      makeTempDirSync();
    } catch (e) {
      if (e.kind === Deno.ErrorKind.PermissionDenied) {
        console.log(firstCheckFailedMessage);
      }
    }
    makeTempDirSync();
  },
  needsEnv: (): void => {
    try {
      env().home;
    } catch (e) {
      if (e.kind === Deno.ErrorKind.PermissionDenied) {
        console.log(firstCheckFailedMessage);
      }
    }
    env().home;
  },
  needsNet: (): void => {
    try {
      listen("tcp", "127.0.0.1:4540");
    } catch (e) {
      if (e.kind === Deno.ErrorKind.PermissionDenied) {
        console.log(firstCheckFailedMessage);
      }
    }
    listen("tcp", "127.0.0.1:4541");
  },
  needsRun: (): void => {
    try {
      const process = run({
        args: [
          "python",
          "-c",
          "import sys; sys.stdout.write('hello'); sys.stdout.flush()"
        ]
      });
    } catch (e) {
      if (e.kind === Deno.ErrorKind.PermissionDenied) {
        console.log(firstCheckFailedMessage);
      }
    }
    const process = run({
      args: [
        "python",
        "-c",
        "import sys; sys.stdout.write('hello'); sys.stdout.flush()"
      ]
    });
  },
  needsPlugins: (): void => {
    try {
      const plugin = openPlugin("some/fake/path");
    } catch (e) {
      if (e.kind === Deno.ErrorKind.PermissionDenied) {
        console.log(firstCheckFailedMessage);
      }
    }
    try {
      const plugin = openPlugin("some/fake/path");
    } catch (e) {
      if (e.kind === Deno.ErrorKind.PermissionDenied) {
        throw e;
      }
    }
  }
}[name];

if (!test) {
  console.log("Unknown test:", name);
  exit(1);
}

test();
