/**
 * Used to enforce illegal constructors
 */
let lock = true;

export function setLock(value: boolean) {
  lock = value;
}

export function getLock(): boolean {
  return lock;
}

