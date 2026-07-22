import type { kind } from "package" with {
  "resolution-mode": "require",
};

const test: typeof kind = "other";
console.log(test);
