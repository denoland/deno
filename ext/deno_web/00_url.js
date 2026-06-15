function getSerialization(url, excludeFragment) {
  // ... existing code ...

  // Handle scheme-relative paths starting with multiple slashes
  if (url.path.startsWith('///')) {
    const scheme = url.protocol.slice(0, -1);
    const path = url.path.slice(2);
    return `${scheme}://${path}`;
  }

  // ... existing code ...
}