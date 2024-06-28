import { sayHello } from "@denotest/a";
import { sayHello as sayHello2 } from "npm:@denotest/a@1";
import { sayHello as sayHello3 } from "npm:@denotest/a@workspace";

sayHello();
sayHello2();
sayHello3();
