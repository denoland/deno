import { main as pb } from "./msg.pb";
import * as dispatch from "./dispatch";

let nextTimerId = 1;

// tslint:disable-next-line:no-any
export type TimerCallback = (...args: any[]) => void;

interface Timer {
  id: number;
  cb: TimerCallback;
  interval: boolean;
  // tslint:disable-next-line:no-any
  args: any[];
  duration: number; // milliseconds
}

const timers = new Map<number, Timer>();

export function initTimers() {
  dispatch.sub("timers", onMessage);
}

function onMessage(payload: Uint8Array) {
  const msg = pb.Msg.decode(payload);
  const { id, done } = msg.timerReady;
  const timer = timers.get(id);
  if (!timer) {
    return;
  }
  timer.cb(...timer.args);
  if (done) {
    timers.delete(id);
  }
}

export function setTimeout(
  cb: TimerCallback,
  duration: number,
  // tslint:disable-next-line:no-any
  ...args: any[]
): number {
  const timer = {
    id: nextTimerId++,
    interval: false,
    duration,
    args,
    cb
  };
  timers.set(timer.id, timer);
  dispatch.sendMsg("timers", {
    timerStart: {
      id: timer.id,
      interval: false,
      duration
    }
  });
  return timer.id;
}

// TODO DRY with setTimeout
export function setInterval(
  cb: TimerCallback,
  repeat: number,
  // tslint:disable-next-line:no-any
  ...args: any[]
): number {
  const timer = {
    id: nextTimerId++,
    interval: true,
    duration: repeat,
    args,
    cb
  };
  timers.set(timer.id, timer);
  dispatch.sendMsg("timers", {
    timerStart: {
      id: timer.id,
      interval: true,
      duration: repeat
    }
  });
  return timer.id;
}

export function clearTimer(id: number) {
  dispatch.sendMsg("timers", {
    timerClear: { id }
  });
}
