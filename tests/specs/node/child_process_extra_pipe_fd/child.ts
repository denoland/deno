import { createWriteStream } from "node:fs";

const ws = createWriteStream(null as unknown as string, { fd: 3 });
ws.write("hello from fd 3\n");
