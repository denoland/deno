import { type B, create } from "./subdir/export_types.ts";

const b: B = create();

console.log(b);
