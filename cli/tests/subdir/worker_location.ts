onmessage = function (): void {
  postMessage(self.location.href);
  close();
};
