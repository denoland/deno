import { bIsMain, isMain as aIsMain } from "./a.ts";

console.log(`main.ts ${import.meta.main}`);
console.log(`a.ts from main.ts ${aIsMain}`);
console.log(`b.ts from main.ts ${bIsMain}`);
