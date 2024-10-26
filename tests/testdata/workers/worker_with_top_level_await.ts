function delay(ms: number) {
  return new Promise<void>((resolve) => {
    setTimeout(() => {
      resolve();
    }, ms);
  });
}

onmessage = (_e: MessageEvent) => {
  postMessage("triggered worker handler");
  close();
};
postMessage("ready");
await delay(1000);
postMessage("never");
