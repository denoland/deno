import { isMain as bIsMain } from "./b.ts";

export const isMain = import.meta.main;
export { bIsMain };
console.log(`a.ts ${import.meta.main}`);
