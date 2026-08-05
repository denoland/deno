import { sayHello as sayHelloVuln1 } from "@denotest/with-vuln1";

export function sayHello() {
  return sayHelloVuln1();
}
