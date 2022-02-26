export function branch(condition: boolean): boolean {
  if (condition) {
    return true;
  } else {
    return false;
  }
}

export function unused(condition: boolean): boolean {
  if (condition) {
    return false;
  } else {
    return true;
  }
}
