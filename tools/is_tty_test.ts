// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { args, exit, isTTY } from "deno";

const name = args[1];
const test = {
  stdin: () => {
    console.log(isTTY().stdin);
  },
  stdout: () => {
    console.log(isTTY().stdout);
  },
  stderr: () => {
    console.log(isTTY().stderr);
  }
}[name];

if (!test) {
  console.log("Unknown test:", name);
  exit(1);
}

test();
