import { sayHello } from "@denotest/a";
import { sayHello as sayHello2 } from "npm:@denotest/a@1";

sayHello();
sayHello2();
