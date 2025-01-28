// @ts-check

/** @import { add } from "http://localhost:4545/add.ts" */

/**
 * @param {typeof add} myValue
 */
export function addHere(myValue) {
  return myValue(1, 2);
}

addHere("");
