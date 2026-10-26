import { getValue, setValue } from "npm:@denotest/esm-basic";
import "npm:@denotest/cjs-default-export";
import type { Fizzbuzz } from "npm:@denotest/types";

const _value: Fizzbuzz = { fizz: "fizz", buzz: "buzz" };
setValue(42);
console.log(getValue());
