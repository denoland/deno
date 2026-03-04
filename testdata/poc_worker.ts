export const sleep = (duration: number) => {
  return new Promise<void>((res) => {
    setTimeout(() => res(), duration);
  });
};

self.onmessage = async (ev) => {
  const ia = ev.data;
  console.log(self.name, ia);
  try {
    const actual = Atomics.compareExchange(ia, 0, 0, 1);

    if (actual === 0) {
      await sleep(0);
      const notified = Atomics.notify(ia, 0);
      console.log(self.name, { actual, notified });
      self.postMessage({ ok: true });
    } else {
      // @ts-expect-error see https://github.com/microsoft/TypeScript/issues/49198
      let { async, value } = Atomics.waitAsync(ia, 0, 1);
      console.log(self.name, { actual, async, value });
      value = await value;
      self.postMessage({ ok: true });
    }
  } catch (err) {
    self.postMessage({ ok: false, err });
  }
};
