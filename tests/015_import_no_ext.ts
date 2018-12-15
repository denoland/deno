import { isTSFile, printHello, phNoExt } from "./subdir/mod3";
console.log(isTSFile);
console.log(printHello);
console.log(phNoExt);

/* TODO Reenable this test and delete the following console.log("true").
import { isMod4 } from "./subdir/mod4";
console.log(isMod4);
*/
console.log("true");

import { printHello as ph } from "http://localhost:4545/tests/subdir/mod2";
console.log(ph);
