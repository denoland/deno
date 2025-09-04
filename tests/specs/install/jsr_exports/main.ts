import { add } from "@denotest/multiple-exports/add";
import { subtract } from "@denotest/multiple-exports/subtract";
import data from "@denotest/multiple-exports/data-json" with { type: "json" };

console.log(add(1, 2));
console.log(subtract(1, 2));
console.log(data);
