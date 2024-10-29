import * as a from "./a.js";
import * as b from "./b.js";
import * as c from "./c.mjs";
import * as d from "./d.mjs";

console.log(a.A);
console.log(b.B2);
console.log(c.C);
console.log(d.D2);

import * as a2 from "./a";
import * as b2 from "./b";
import * as c2 from "./c";
import * as d2 from "./d";

console.log(a2.A);
console.log(b2.B2);
console.log(c2.C);
console.log(d2.D2);

import * as dirTs from "./dir_ts";
import * as dirJs from "./dir_js";
import * as dirMts from "./dir_mts";
import * as dirMjs from "./dir_mjs";

console.log(dirTs.Dir);
console.log(dirJs.Dir2);
console.log(dirMts.Dir);
console.log(dirMjs.Dir2);
