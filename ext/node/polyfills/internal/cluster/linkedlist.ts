// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Ports lib/internal/linkedlist.js (only the small subset cluster uses).

// deno-lint-ignore-file no-explicit-any

export function init(list: any) {
  list._idleNext = list;
  list._idlePrev = list;
  return list;
}

export function peek(list: any) {
  if (list._idlePrev === list) return null;
  return list._idlePrev;
}

export function remove(item: any) {
  if (item._idleNext) {
    item._idleNext._idlePrev = item._idlePrev;
  }
  if (item._idlePrev) {
    item._idlePrev._idleNext = item._idleNext;
  }
  item._idleNext = null;
  item._idlePrev = null;
}

export function append(list: any, item: any) {
  if (item._idleNext || item._idlePrev) {
    remove(item);
  }
  item._idleNext = list._idleNext;
  item._idlePrev = list;
  list._idleNext._idlePrev = item;
  list._idleNext = item;
}

export function isEmpty(list: any) {
  return list._idleNext === list;
}
