/// <reference types="node" />
import { process as test } from "node:process";

{
  const value: number = test.getValue();
  console.log(value);
}
{
  const value: number = process.getValue();
  console.log(value);
}
