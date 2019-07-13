// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { Event } from "./event";
import { window } from "./window";
/** Triggers `load` event */
export function triggerLoadEvent(): void {
  const onload = window.onload;
  if (typeof onload === "function") {
    onload(new Event("load"));
  }
}
