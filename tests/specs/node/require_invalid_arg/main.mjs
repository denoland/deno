import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

try {
  require(123);
} catch (e) {
  console.log(e.code, e.constructor.name);
}

try {
  require("");
} catch (e) {
  console.log(e.code, e.constructor.name);
}
