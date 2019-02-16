// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// tslint:disable-next-line:no-reference
/// <reference path="./const.d.ts" />

interface Version {
  deno: string | null;
  v8: string | null;
  typescript: string;
}

export const version: Version = {
  deno: null,
  v8: null,
  typescript: TS_VERSION
};
