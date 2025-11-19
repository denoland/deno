// Simple test to verify the socket_dev server implementation
// This should be run with the test server running on port 4263

const SOCKET_DEV_PORT = 4263;

async function testSocketDevServer() {
  const testCases = [
    {
      name: "Valid npm package",
      purl: "pkg:npm/lodash@4.17.21",
      shouldPass: true,
    },
    {
      name: "Scoped package",
      purl: "pkg:npm/@types/node@18.0.0",
      shouldPass: true,
    },
    {
      name: "Invalid URL (no purl prefix)",
      url: "/invalid/path",
      shouldPass: false,
    },
    {
      name: "Malformed purl (no version)",
      purl: "pkg:npm/lodash",
      shouldPass: false,
    },
    {
      name: "Malformed purl (wrong type)",
      purl: "pkg:cargo/some-package@1.0.0",
      shouldPass: false,
    },
  ];

  console.log("Testing socket_dev server on port", SOCKET_DEV_PORT);
  console.log("=".repeat(60));

  for (const testCase of testCases) {
    const url = testCase.url ||
      `http://localhost:${SOCKET_DEV_PORT}/purl/${
        encodeURIComponent(testCase.purl)
      }`;

    try {
      const response = await fetch(url);

      if (testCase.shouldPass) {
        if (response.status === 200) {
          const data = await response.json();
          console.log(`✓ ${testCase.name}`);
          console.log(`  Response:`, data);

          // Verify response structure
          if (testCase.purl) {
            const parts = testCase.purl.split("/")[1].split("@");
            const expectedName = parts[0];
            const expectedVersion = parts[parts.length - 1];

            if (data.name !== expectedName) {
              console.log(
                `  ⚠️  Expected name ${expectedName}, got ${data.name}`,
              );
            }
            if (data.version !== expectedVersion) {
              console.log(
                `  ⚠️  Expected version ${expectedVersion}, got ${data.version}`,
              );
            }
          }
        } else {
          console.log(`✗ ${testCase.name}`);
          console.log(`  Expected 200, got ${response.status}`);
        }
      } else {
        if (response.status === 404) {
          console.log(`✓ ${testCase.name} (correctly returned 404)`);
        } else {
          console.log(`✗ ${testCase.name}`);
          console.log(`  Expected 404, got ${response.status}`);
        }
      }
    } catch (error) {
      console.log(`✗ ${testCase.name}`);
      console.log(`  Error:`, error.message);
    }
    console.log("");
  }
}

testSocketDevServer();
