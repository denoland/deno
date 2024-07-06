import * as a1 from "@denotest/a";
import * as a2 from "npm:@denotest/a@1";
import * as a3 from "npm:@denotest/a@workspace";
import * as c from "@denotest/c";

a1.sayHello();
a2.sayHello();
a3.sayHello();
c.sayHello();
