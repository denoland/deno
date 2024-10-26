onmessage = function () {
  postMessage(
    [
      typeof navigator.language,
      typeof navigator.languages,
      typeof navigator.userAgent,
      typeof navigator.hardwareConcurrency,
    ].join(", "),
  );
  close();
};
