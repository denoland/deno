import { readFile } from "fs/promises";

console.log(await readFile("./test.txt"));
