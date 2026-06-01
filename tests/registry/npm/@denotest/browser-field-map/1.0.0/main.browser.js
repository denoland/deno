import { greet } from "./util.js";
import fs from "fs";
import foo from "foo";
import * as ext from "./extensionless.js";
import * as dirext from "./dir-extensionless/index.js";
export const hello = () =>
  "browser: " + greet() +
  " fs=" + typeof fs +
  " foo=" + foo +
  " ext=" + JSON.stringify(ext) +
  " dirext=" + JSON.stringify(dirext);
