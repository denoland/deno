import { _global } from "./util";
import { sendMsgFromObject } from "./os";

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

export function setTimeout(cb: TimerCallback, duration: number): number {
  const timer = {
    id: nextTimerId++,
    interval: false,
    duration,
    cb
  };
  timers.set(timer.id, timer);
  sendMsgFromObject({
    timerStart: {
      id: timer.id,
      interval: false,
      duration
    }
  });
  return timer.id;
}
_global["setTimeout"] = setTimeout;

export function timerReady(id: number, done: boolean): void {
  const timer = timers.get(id);
  timer.cb();
  if (done) {
    timers.delete(id);
  }
}
