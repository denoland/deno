// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { args, exit, isTTY } from "deno";

const name = args[1];
const test = {
  stdin: () => {
    console.log(isTTY(0));
  },
  stdout: () => {
    console.log(isTTY(1));
  },
  stderr: () => {
    console.log(isTTY(2));
  },
  file: () => {
    console.log(isTTY(10));
  }
}[name];

if (!test) {
  console.log("Unknown test:", name);
  exit(1);
}

test();
