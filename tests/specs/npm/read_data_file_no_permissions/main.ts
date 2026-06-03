import { readData } from "npm:@denotest/read-data-file";

// Reading a data file bundled inside an npm package should work without
// `--allow-read` (https://github.com/denoland/deno/issues/18607).
console.log(readData());
