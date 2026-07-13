// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const name = Deno.args[0];
// deno-lint-ignore no-explicit-any
const test: { [key: string]: (...args: any[]) => void | Promise<void> } = {
  readRequired() {
    Deno.readFileSync("assets/hello.txt");
    return Promise.resolve();
  },
  writeRequired() {
    Deno.makeTempDirSync();
  },
  envRequired() {
    // Use a variable that is not on the default-readable env allowlist so this
    // still requires --allow-env. ("home" case-folds to the allowlisted "HOME"
    // on Windows.)
    Deno.env.get("NOT_ALLOWLISTED_ENV_VAR");
  },
  netRequired() {
    Deno.listen({ transport: "tcp", port: 4541 });
  },
  async runRequired() {
    await new Deno.Command(Deno.build.os === "windows" ? "cmd.exe" : "printf", {
      args: Deno.build.os === "windows" ? ["/c", "echo hello"] : ["hello"],
    }).output();
  },
};

if (!test[name]) {
  console.log("Unknown test:", name);
  Deno.exit(1);
}

test[name]();
