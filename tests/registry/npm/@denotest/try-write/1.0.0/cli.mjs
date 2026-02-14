#!/usr/bin/env node
import fs from "node:fs";
fs.writeFileSync("test.txt", "hello");
console.log("write succeeded");
