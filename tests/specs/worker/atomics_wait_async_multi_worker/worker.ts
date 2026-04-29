self.onmessage = async (ev) => {
  const ia: Int32Array = ev.data;
  try {
    const { value } = Atomics.waitAsync(ia, 0, 0);
    self.postMessage("waiting");
    const result = await value;
    self.postMessage({ ok: true, result });
  } catch (err) {
    self.postMessage({ ok: false, err: String(err) });
  }
};
