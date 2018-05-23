import { main as pb } from "./msg.pb";
import * as dispatch from "./dispatch";

let nextTimerId = 1;

// tslint:disable-next-line:no-any
type TimerCallback = (...args: any[]) => void;

interface Timer {
  id: number;
  cb: TimerCallback;
  interval: boolean;
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
  timer.cb();
  if (done) {
    timers.delete(id);
  }
}

export function setTimeout(cb: TimerCallback, duration: number): number {
  const timer = {
    id: nextTimerId++,
    interval: false,
    duration,
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
