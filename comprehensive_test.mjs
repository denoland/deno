// Comprehensive test for all three Node.js compatibility fixes
console.log('🧪 Testing Node.js Compatibility Fixes\n');

// Test 1: assert.deepStrictEqual with Number objects
console.log('1️⃣  Testing assert.deepStrictEqual with Number objects');
try {
  const assert = await import('node:assert/strict');
  
  // Should throw for different values
  try {
    assert.deepStrictEqual(new Number(1), new Number(2));
    console.log('❌ FAIL: Should have thrown AssertionError');
  } catch (e) {
    if (e.name === 'AssertionError') {
      console.log('✅ PASS: Correctly threw AssertionError for different Number objects');
    } else {
      console.log('❌ FAIL: Threw wrong error type:', e.name);
    }
  }

  // Should pass for same values
  try {
    assert.deepStrictEqual(new Number(1), new Number(1));
    console.log('✅ PASS: Same Number objects are equal');
  } catch (e) {
    console.log('❌ FAIL: Same Number objects should be equal');
  }
} catch (e) {
  console.log('❌ FAIL: Error importing assert/strict:', e.message);
}

// Test 2: partialDeepStrictEqual method
console.log('\n2️⃣  Testing partialDeepStrictEqual method');
try {
  const assert = await import('node:assert');
  
  // Check method exists
  if (typeof assert.partialDeepStrictEqual === 'function') {
    console.log('✅ PASS: partialDeepStrictEqual method exists');
    
    // Test array subset functionality
    try {
      assert.partialDeepStrictEqual([1, 2, 3, 4, 5], [2, 4]);
      console.log('✅ PASS: Array subset check works');
    } catch (e) {
      console.log('❌ FAIL: Array subset check failed:', e.message);
    }

    // Test object subset functionality
    try {
      assert.partialDeepStrictEqual({a: 1, b: 2, c: 3}, {a: 1, c: 3});
      console.log('✅ PASS: Object subset check works');
    } catch (e) {
      console.log('❌ FAIL: Object subset check failed:', e.message);
    }

    // Test should fail case
    try {
      assert.partialDeepStrictEqual([1, 2, 3], [4, 5]);
      console.log('❌ FAIL: Should have thrown for non-subset');
    } catch (e) {
      console.log('✅ PASS: Correctly threw for non-subset');
    }
  } else {
    console.log('❌ FAIL: partialDeepStrictEqual method missing');
  }
} catch (e) {
  console.log('❌ FAIL: Error importing assert:', e.message);
}

// Test 3: writeEarlyHints method
console.log('\n3️⃣  Testing writeEarlyHints method');
try {
  const http = await import('node:http');
  let callbackExecuted = false;
  let testCompleted = false;

  const server = http.createServer((req, res) => {
    // Test method exists
    if (typeof res.writeEarlyHints === 'function') {
      console.log('✅ PASS: writeEarlyHints method exists');
    } else {
      console.log('❌ FAIL: writeEarlyHints method missing');
      res.end();
      return;
    }

    // Test method execution
    try {
      res.writeEarlyHints({
        'link': '</styles.css>; rel=preload; as=style'
      }, () => {
        callbackExecuted = true;
        console.log('✅ PASS: writeEarlyHints callback executed');
      });
      
      console.log('✅ PASS: writeEarlyHints executed without error');
    } catch (e) {
      console.log('❌ FAIL: writeEarlyHints threw error:', e.message);
    }

    res.writeHead(200);
    res.end('Hello World');
    
    setTimeout(() => {
      if (callbackExecuted) {
        console.log('✅ PASS: Callback was executed asynchronously');
      } else {
        console.log('❌ FAIL: Callback was not executed');
      }
      server.close();
      testCompleted = true;
      
      // Final summary
      setTimeout(() => {
        console.log('\n🎉 All tests completed!');
        console.log('📝 If all tests show ✅ PASS, the fixes are working correctly.');
      }, 50);
    }, 100);
  });

  server.listen(8001, () => {
    // Make a request to trigger the handler
    const req = http.request('http://localhost:8001', () => {});
    req.on('error', () => {}); // Ignore connection errors
    req.end();
  });

  // Timeout fallback
  setTimeout(() => {
    if (!testCompleted) {
      console.log('⚠️  HTTP test timed out - this may indicate an issue');
      server.close();
    }
  }, 5000);

} catch (e) {
  console.log('❌ FAIL: Error importing http:', e.message);
}