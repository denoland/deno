import doRequire from "package";
import path from "node:path";

doRequire(path.resolve(import.meta.dirname, "file.js"));
doRequire(path.resolve(import.meta.dirname, "logs_require.js"));
