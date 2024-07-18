// should resolve these as bare specifiers within the workspace
import * as a from "@denotest/a";
import * as c from "@denotest/c";

a.sayHello();
c.sayHello();
