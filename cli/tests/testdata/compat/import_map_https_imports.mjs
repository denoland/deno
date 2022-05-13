import { sortBy } from "std/collections/sort_by.ts";
import { findSingle } from "https://deno.land/std@0.126.0/collections/find_single.ts";
import os from "node:os";

console.log(sortBy([2, 3, 1], (it) => it));
console.log(findSingle([2, 3, 1], (it) => it == 2));
console.log("arch", os.arch());
