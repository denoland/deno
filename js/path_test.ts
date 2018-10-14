// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual } from "./test_util.ts";
import * as deno from "deno";

test(function pathBackwardsSuccess() {
  let path = "/foo/bar/test";
  path = deno.pathBackwards(path);
  assertEqual(path, "\\foo\\bar\\test");
  path = "/////foo///bar/////test";
  path = deno.pathBackwards(path);
  assertEqual(path, "\\foo\\bar\\test");
  path = "///foo/bar///test///";
  path = deno.pathBackwards(path);
  assertEqual(path, "\\foo\\bar\\test\\");
});

test(function pathForwardsSuccess() {
  let path = "C:\\foo\\bar\\test";
  path = deno.pathForwards(path);
  assertEqual(path, "/foo/bar/test");
  path = "C:\\\\foo\\\\bar\\test";
  path = deno.pathForwards(path);
  assertEqual(path, "/foo/bar/test");
  path = "C:\\\\foo\\\\bar\\test\\";
  path = deno.pathForwards(path);
  assertEqual(path, "/foo/bar/test/");
});