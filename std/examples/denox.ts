// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const {
  run,
  execPath,
  env,
  cwd,
  args,
  stdin,
  stdout,
  stderr,
  exit,
  makeTempDirSync,
  removeSync,
  Signal
} = Deno;

// TODO: handle signal

function filterArgs(args: string[]): string[] {
  const newArgs = [];
  for (const arg of args) {
    if (arg !== "--") {
      newArgs.push(arg);
    }
  }
  return newArgs;
}

const tempDir = makeTempDirSync({
  prefix: "denox_"
});

const ps = run({
  cwd: cwd(),
  args: filterArgs([execPath()].concat(args.slice(1))),
  env: {
    ...env(),
    DENO_DIR: tempDir
  },
  stdin: "inherit",
  stdout: "inherit",
  stderr: "inherit"
});

const status = await ps.status();

removeSync(tempDir, { recursive: true });

exit(status.code);
