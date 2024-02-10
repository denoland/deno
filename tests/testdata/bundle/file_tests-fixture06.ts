import { isMain, modUrl } from "./subdir/f.ts";

console.log(isMain, modUrl);
console.log(import.meta.main, import.meta.url);
