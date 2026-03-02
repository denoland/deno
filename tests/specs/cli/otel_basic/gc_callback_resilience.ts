// Copyright 2018-2026 the Deno authors. MIT license.

// Test that GC callbacks handle missing isolate slots gracefully
// This tests the fix for isolate slot panic during cleanup

console.log("starting gc callback resilience test");

// Force garbage collection multiple times during telemetry operations
// This should not crash even if slots are missing during cleanup
for (let i = 0; i < 10; i++) {
  // Create some objects that will need GC
  const objects = [];
  for (let j = 0; j < 1000; j++) {
    objects.push({ data: `test-${i}-${j}`, nested: { value: j } });
  }
  
  // Force GC to trigger the callbacks
  if (globalThis.gc) {
    globalThis.gc();
  }
  
  // Clear references
  objects.length = 0;
}

// The test passes if we get here without crashing
// GC callbacks should handle missing isolate slots gracefully

console.log("gc callback resilience test completed");