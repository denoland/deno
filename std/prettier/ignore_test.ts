// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, runIfMain } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./ignore.ts";

const testCases = [
  {
    input: `# this is a comment
node_modules
`,
    output: new Set(["node_modules"])
  },
  {
    input: ` # invalid comment
`,
    output: new Set([" # invalid comment"])
  },
  {
    input: `
node_modules
package.json
`,
    output: new Set(["node_modules", "package.json"])
  },
  {
    input: `
    node_modules
    package.json
`,
    output: new Set(["    node_modules", "    package.json"])
  },
  {
    input: `*.orig
*.pyc
*.swp

/.idea/
/.vscode/
gclient_config.py_entries
/gh-pages/
/target/

# Files that help ensure VSCode can work but we don't want checked into the
# repo
/node_modules
/tsconfig.json

# We use something stronger than lockfiles, we have all NPM modules stored in a
# git. We do not download from NPM during build.
# https://github.com/denoland/deno_third_party
yarn.lock
# yarn creates this in error.
tools/node_modules/
    `,
    output: new Set([
      "*.orig",
      "*.pyc",
      "*.swp",
      "/.idea/",
      "/.vscode/",
      "gclient_config.py_entries",
      "/gh-pages/",
      "/target/",
      "/node_modules",
      "/tsconfig.json",
      "yarn.lock",
      "tools/node_modules/"
    ])
  }
];

test({
  name: "[encoding.ignore] basic",
  fn(): void {
    for (const { input, output } of testCases) {
      assertEquals(parse(input), output);
    }
  }
});

runIfMain(import.meta);
