// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
import { main as pb } from "./msg.pb";
import * as dispatch from "./dispatch";
import { assert } from "./util";

let nextTimerId = 1;

// tslint:disable-next-line:no-any
export type TimerCallback = (...args: any[]) => void;

interface Timer {
  id: number;
  cb: TimerCallback;
  interval: boolean;
  // tslint:disable-next-line:no-any
  args: any[];
  delay: number; // milliseconds
}

const timers = new Map<number, Timer>();

export function initTimers() {
  dispatch.sub("timers", onMessage);
}

function onMessage(payload: Uint8Array) {
  const msg = pb.Msg.decode(payload);
  assert(msg.command === pb.Msg.Command.TIMER_READY);
  const id = msg.timerReadyId;
  const done = msg.timerReadyDone;
  const timer = timers.get(id);
  if (!timer) {
    return;
  }
  timer.cb(...timer.args);
  if (done) {
    timers.delete(id);
  }
}

function setTimer(
  cb: TimerCallback,
  delay: number,
  interval: boolean,
  // tslint:disable-next-line:no-any
  args: any[]
): number {
  const timer = {
    id: nextTimerId++,
    interval,
    delay,
    args,
    cb
  };
  timers.set(timer.id, timer);
  dispatch.sendMsg("timers", {
    command: pb.Msg.Command.TIMER_START,
    timerStartId: timer.id,
    timerStartInterval: timer.interval,
    timerStartDelay: timer.delay
  });
  return timer.id;
}

export function setTimeout(
  cb: TimerCallback,
  delay: number,
  // tslint:disable-next-line:no-any
  ...args: any[]
): number {
  return setTimer(cb, delay, false, args);
}

export function setInterval(
  cb: TimerCallback,
  delay: number,
  // tslint:disable-next-line:no-any
  ...args: any[]
): number {
  return setTimer(cb, delay, true, args);
}

export function clearTimer(id: number) {
  dispatch.sendMsg("timers", {
    command: pb.Msg.Command.TIMER_CLEAR,
    timerClearId: id
  });
}
