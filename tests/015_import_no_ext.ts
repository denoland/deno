import { isTSFile, printHello, phNoExt } from "./subdir/mod3";
console.log(isTSFile);
console.log(printHello);
console.log(phNoExt);

import { isMod4 } from "./subdir/mod4";
console.log(isMod4);

import { printHello as ph } from "http://localhost:4545/tests/subdir/mod2";
console.log(ph);
