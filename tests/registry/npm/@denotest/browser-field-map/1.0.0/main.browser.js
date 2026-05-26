import { greet } from "./util.js";
import fs from "fs";
import foo from "foo";
export const hello = () =>
  "browser: " + greet() + " fs=" + typeof fs + " foo=" + foo;
