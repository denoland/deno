import { create, type B } from "./subdir/export_types.ts";

const b: B = create();

console.log(b);
