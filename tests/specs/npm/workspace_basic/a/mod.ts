import { getValue, setValue } from "@denotest/esm-basic";

export function sayHello() {
  setValue(5);
  console.log("Hello", getValue());
}
