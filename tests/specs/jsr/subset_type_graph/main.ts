import { Foo as Foo1 } from "jsr:@denotest/subset-type-graph@0.1.0";
import { Foo as Foo2 } from "jsr:@denotest/subset-type-graph-invalid@0.1.0";

// these will both raise type checking errors
const error1: string = new Foo1().method();
const error2: string = new Foo2().method();
console.log(error1);
console.log(error2);

// now raise some errors that will show the original code and
// these should source map to the original
new Foo1().method2();
new Foo2().method2();
