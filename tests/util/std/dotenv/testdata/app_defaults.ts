import { load } from "../mod.ts";
const conf = await load();

console.log(JSON.stringify(conf, null, 2));
