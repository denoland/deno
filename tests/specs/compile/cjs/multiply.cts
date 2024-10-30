/// <reference types="npm:@types/node" />
exports.multiply = function (a: number, b: number): number {
  return require("./divide.cts").divide(a, 1 / b);
};
