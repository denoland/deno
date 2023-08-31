// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";
import { nextTick } from "node:process";

type Subscriber = (message: unknown, name?: string) => void;

export class Channel {
  _subscribers: Subscriber[];
  name: string;
  constructor(name: string) {
    this._subscribers = [];
    this.name = name;
  }

  publish(message: unknown) {
    for (const subscriber of this._subscribers) {
      try {
        subscriber(message, this.name);
      } catch (err) {
        nextTick(() => {
          throw err;
        });
      }
    }
  }

  subscribe(subscription: Subscriber) {
    validateFunction(subscription, "subscription");

    this._subscribers.push(subscription);
  }

  unsubscribe(subscription: Subscriber) {
    if (!this._subscribers.includes(subscription)) {
      return false;
    }

    this._subscribers.splice(this._subscribers.indexOf(subscription), 1);

    return true;
  }

  get hasSubscribers() {
    return this._subscribers.length > 0;
  }
}

const channels: Record<string, Channel> = {};

export function channel(name: string) {
  if (typeof name !== "string" && typeof name !== "symbol") {
    throw new ERR_INVALID_ARG_TYPE("channel", ["string", "symbol"], name);
  }

  if (!Object.hasOwn(channels, name)) {
    channels[name] = new Channel(name);
  }

  return channels[name];
}

export function hasSubscribers(name: string) {
  if (!Object.hasOwn(channels, name)) {
    return false;
  }

  return channels[name].hasSubscribers;
}

export function subscribe(name: string, subscription: Subscriber) {
  const c = channel(name);

  return c.subscribe(subscription);
}

export function unsubscribe(name: string, subscription: Subscriber) {
  const c = channel(name);

  return c.unsubscribe(subscription);
}

export default {
  channel,
  hasSubscribers,
  subscribe,
  unsubscribe,
  Channel,
};
