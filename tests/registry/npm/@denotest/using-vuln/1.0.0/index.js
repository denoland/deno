import { sayHello as sayHelloVuln2 } from "@denotest/with-vuln2";

export function sayHello() {
  return sayHelloVuln2();
}