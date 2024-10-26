import { sayHello } from "./index.js";
console.log(sayHello());
import path from "node:path";
import fs from "node:fs";
fs.writeSync(fs.openSync(path.join(process.env.INIT_CWD, "say-hello-output.txt"), "w"), sayHello());
