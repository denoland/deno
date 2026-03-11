const sleep = (duration: number) => {
  return new Promise<void>((res) => {
    setTimeout(() => res(), duration);
  });
};

self.onmessage = async (ev) => {
  const ia = ev.data;
  try {
    const actual = Atomics.compareExchange(ia, 0, 0, 1);

    if (actual === 0) {
      await sleep(0);
      Atomics.notify(ia, 0);
      self.postMessage({ ok: true });
    } else {
      let { value } = Atomics.waitAsync(ia, 0, 1);
      value = await value;
      self.postMessage({ ok: true });
    }
  } catch (err) {
    self.postMessage({ ok: false, err });
  }
};
