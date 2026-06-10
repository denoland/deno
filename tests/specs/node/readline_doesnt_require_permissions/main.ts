import { Readable } from "node:stream";
import { createInterface } from "node:readline";

const input = Readable.from(`
    l1
    l2
    l3
`);

const stream = createInterface({ input });

const lines = await Array.fromAsync(stream, (str) => str.trim());

console.log(lines.filter(Boolean));
