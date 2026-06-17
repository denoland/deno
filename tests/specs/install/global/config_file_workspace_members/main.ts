import { greet } from "@scope/greet";
import { upper } from "@scope/greet/upper";
import { exclaim } from "@app/util";
console.log(exclaim(upper(greet("world"))));
