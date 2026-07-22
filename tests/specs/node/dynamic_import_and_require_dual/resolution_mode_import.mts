import type { kind } from "package" with {
  "resolution-mode": "import",
};

const test: typeof kind = "other";
console.log(test);
