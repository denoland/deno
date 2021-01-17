// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { bgBlue, bold, italic, red } from "../fmt/colors.ts";

if (import.meta.main) {
  console.log(bgBlue(italic(red(bold("Hello world!")))));
}
