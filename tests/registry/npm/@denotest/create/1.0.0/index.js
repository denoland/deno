import { add } from "@denotest/add";
import * as fs from "fs";

const fileName = `./${add(1, 2)}`;
fs.writeFileSync(fileName, "test");
console.log("Initialized!");
