/// <reference types="./subdir/emittable.d.ts" />

import "./subdir/polyfill.ts";

export const a = "a";

console.log(globalThis.polyfill);
