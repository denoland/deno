// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { assert } from "./util";
import * as util from "./util";
import { deno as fbs } from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import { libdeno } from "./libdeno";

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

/** @internal */
export function onMessage(msg: fbs.TimerReady) {
  const timerReadyId = msg.id();
  const timerReadyDone = msg.done();
  const timer = timers.get(timerReadyId);
  if (!timer) {
    return;
  }
  timer.cb(...timer.args);
  if (timerReadyDone) {
    timers.delete(timerReadyId);
  }
}

function startTimer(
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

  util.log("timers.ts startTimer");

  // Send TimerStart message
  const builder = new flatbuffers.Builder();
  fbs.TimerStart.startTimerStart(builder);
  fbs.TimerStart.addId(builder, timer.id);
  fbs.TimerStart.addInterval(builder, timer.interval);
  fbs.TimerStart.addDelay(builder, timer.delay);
  const msg = fbs.TimerStart.endTimerStart(builder);
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.TimerStart);
  builder.finish(fbs.Base.endBase(builder));
  const resBuf = libdeno.send(builder.asUint8Array());
  assert(resBuf == null);

  return timer.id;
}

export function setTimeout(
  cb: TimerCallback,
  delay: number,
  // tslint:disable-next-line:no-any
  ...args: any[]
): number {
  return startTimer(cb, delay, false, args);
}

export function setInterval(
  cb: TimerCallback,
  delay: number,
  // tslint:disable-next-line:no-any
  ...args: any[]
): number {
  return startTimer(cb, delay, true, args);
}

export function clearTimer(id: number) {
  timers.delete(id);

  const builder = new flatbuffers.Builder();
  fbs.TimerClear.startTimerClear(builder);
  fbs.TimerClear.addId(builder, id);
  const msg = fbs.TimerClear.endTimerClear(builder);
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.TimerClear);
  builder.finish(fbs.Base.endBase(builder));
  const resBuf = libdeno.send(builder.asUint8Array());
  assert(resBuf == null);
}
