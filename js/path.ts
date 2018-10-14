// Copyright 2018 the Deno authors. All rights reserved. MIT license.

export function pathBackwards(path: string): string {
  path.trim();
  path = path.replace(/\//g, "\\");
  const double = /\\\\/;
  while (double.test(path)) {
    path = path.replace(double, "\\");
  }
  return path;
}

export function pathForwards(path: string): string {
  path.trim();
  path = path.replace(/^([a-zA-Z]+:|\.\/)/, "");
  path = path.replace(/\\/g, "/");
  const double = /\/\//;
  while (double.test(path)) {
    path = path.replace(double, "/");
  }
  return path;
}