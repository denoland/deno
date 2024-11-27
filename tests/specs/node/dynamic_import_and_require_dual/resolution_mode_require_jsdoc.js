// @ts-check

/** @import { kind } from "package" with { 'resolution-mode': 'require' } */

/**
 * @param {typeof kind} myValue
 */
export function log(myValue) {
  console.log(myValue);
}

log("value");
