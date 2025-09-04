import { foo } from "./export_type_def.ts";

function bar(a: number) {
  console.log(a);
}

bar(foo);
