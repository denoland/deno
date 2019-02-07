// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { bgBlue, red, bold, italic } from "./mod.ts";

console.log(bgBlue(italic(red(bold("Hello world!")))));
