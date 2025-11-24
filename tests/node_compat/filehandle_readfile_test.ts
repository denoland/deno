// Copyright 2018-2025 the Deno authors. MIT license.

/**
 * Comprehensive test suite for FileHandle.readFile(options) method
 * Tests Node.js compatibility for reading files with encoding options
 * and verifying that file handles remain open after reading.
 */

import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";
import { assert } from "@std/assert";
import { assertEquals } from "@std/assert";

const testContent = "Hello Deno World";
const testContentBytes = new TextEncoder().encode(testContent);

/**
 * Test Case 1: Read file without encoding (returns Buffer)
 */
Deno.test({
  name: "FileHandle.readFile() - No encoding returns Buffer",
  async fn() {
    // Setup: Create temporary file with known content
    const tempFile = await Deno.makeTempFile();
    await Deno.writeTextFile(tempFile, testContent);

    try {
      // Open the file using fs.promises.open()
      const fileHandle = await fs.open(tempFile, "r");

      // Call filehandle.readFile() without options
      const result = await fileHandle.readFile();

      // Assert: Verify the result is a Buffer
      assert(result instanceof Buffer, "Result should be a Buffer");

      // Assert: Verify content matches the known bytes
      assertEquals(
        result,
        Buffer.from(testContentBytes),
        "Buffer content should match original bytes",
      );

      // Post-Read State Test: Verify file handle remains open and usable
      const stat = await fileHandle.stat();
      assert(
        stat.isFile(),
        "File handle should still be usable after readFile",
      );
      assertEquals(
        stat.size,
        testContent.length,
        "File size should be accessible",
      );

      // Cleanup: Close the file handle
      await fileHandle.close();
    } finally {
      // Cleanup: Remove temporary file
      await Deno.remove(tempFile);
    }
  },
});

/**
 * Test Case 2: Read file with encoding (returns string)
 */
Deno.test({
  name: "FileHandle.readFile({ encoding: 'utf8' }) - Returns string",
  async fn() {
    // Setup: Create temporary file with known content
    const tempFile = await Deno.makeTempFile();
    await Deno.writeTextFile(tempFile, testContent);

    try {
      // Open the file using fs.promises.open()
      const fileHandle = await fs.open(tempFile, "r");

      // Call filehandle.readFile() with encoding option
      const result = await fileHandle.readFile({ encoding: "utf8" });

      // Assert: Verify the result is a string
      assertEquals(
        typeof result,
        "string",
        "Result should be a string when encoding is specified",
      );

      // Assert: Verify content matches the known string
      assertEquals(
        result,
        testContent,
        "String content should match original text",
      );

      // Post-Read State Test: Verify file handle remains open and usable
      const stat = await fileHandle.stat();
      assert(
        stat.isFile(),
        "File handle should still be usable after readFile",
      );
      assertEquals(
        stat.size,
        testContent.length,
        "File size should be accessible",
      );

      // Cleanup: Close the file handle
      await fileHandle.close();
    } finally {
      // Cleanup: Remove temporary file
      await Deno.remove(tempFile);
    }
  },
});

/**
 * Test Case 3: Read file with encoding as string parameter
 */
Deno.test({
  name: "FileHandle.readFile('utf8') - Encoding as string parameter",
  async fn() {
    // Setup: Create temporary file with known content
    const tempFile = await Deno.makeTempFile();
    await Deno.writeTextFile(tempFile, testContent);

    try {
      // Open the file
      const fileHandle = await fs.open(tempFile, "r");

      // Call readFile with encoding as string parameter
      const result = await fileHandle.readFile("utf8");

      // Assert: Verify the result is a string
      assertEquals(typeof result, "string", "Result should be a string");

      // Assert: Verify content matches
      assertEquals(result, testContent, "Content should match");

      // Verify handle is still open
      const stat = await fileHandle.stat();
      assert(stat.isFile(), "File handle should remain open");

      await fileHandle.close();
    } finally {
      await Deno.remove(tempFile);
    }
  },
});

/**
 * Test Case 4: Multiple operations after readFile
 */
Deno.test({
  name: "FileHandle.readFile() - Multiple operations after read",
  async fn() {
    // Setup: Create temporary file
    const tempFile = await Deno.makeTempFile();
    await Deno.writeTextFile(tempFile, testContent);

    try {
      const fileHandle = await fs.open(tempFile, "r+");

      // First readFile
      const data1 = await fileHandle.readFile({ encoding: "utf8" });
      assertEquals(data1, testContent, "First read should succeed");

      // Verify we can still perform stat
      const stat1 = await fileHandle.stat();
      assert(stat1.isFile(), "Stat should work after first read");

      // Verify we can read again (from current position, which is EOF)
      const data2 = await fileHandle.readFile({ encoding: "utf8" });
      assertEquals(
        data2,
        "",
        "Second read from EOF should return empty string",
      );

      // Verify we can still perform stat again
      const stat2 = await fileHandle.stat();
      assert(stat2.isFile(), "Stat should work after second read");

      await fileHandle.close();
    } finally {
      await Deno.remove(tempFile);
    }
  },
});

/**
 * Test Case 5: Read from current position after partial read
 */
Deno.test({
  name: "FileHandle.readFile() - Reads from current position",
  async fn() {
    // Setup: Create temporary file
    const tempFile = await Deno.makeTempFile();
    await Deno.writeTextFile(tempFile, "ABCDEFGHIJ");

    try {
      const fileHandle = await fs.open(tempFile, "r");

      // Read first 5 bytes using read()
      const buffer = Buffer.alloc(5);
      const { bytesRead } = await fileHandle.read(buffer, 0, 5, null);
      assertEquals(bytesRead, 5, "Should read 5 bytes");
      assertEquals(buffer.toString(), "ABCDE", "First 5 bytes should be ABCDE");

      // Now readFile should read from position 5 onwards
      const remaining = await fileHandle.readFile({ encoding: "utf8" });
      assertEquals(
        remaining,
        "FGHIJ",
        "readFile should read from current position",
      );

      // Verify handle is still open
      const stat = await fileHandle.stat();
      assert(stat.isFile(), "File handle should remain open");

      await fileHandle.close();
    } finally {
      await Deno.remove(tempFile);
    }
  },
});

/**
 * Test Case 6: Read empty file
 */
Deno.test({
  name: "FileHandle.readFile() - Empty file",
  async fn() {
    // Setup: Create empty temporary file
    const tempFile = await Deno.makeTempFile();

    try {
      const fileHandle = await fs.open(tempFile, "r");

      // Read empty file as Buffer
      const bufferResult = await fileHandle.readFile();
      assert(bufferResult instanceof Buffer, "Result should be a Buffer");
      assertEquals(bufferResult.length, 0, "Buffer should be empty");

      // Verify handle is still open
      const stat = await fileHandle.stat();
      assert(stat.isFile(), "File handle should remain open");

      await fileHandle.close();
    } finally {
      await Deno.remove(tempFile);
    }
  },
});

/**
 * Test Case 7: Read with different encodings
 */
Deno.test({
  name: "FileHandle.readFile() - Different encodings",
  async fn() {
    const tempFile = await Deno.makeTempFile();
    await Deno.writeTextFile(tempFile, testContent);

    try {
      // Test ASCII encoding
      let fileHandle = await fs.open(tempFile, "r");
      let result = await fileHandle.readFile({ encoding: "ascii" });
      assertEquals(typeof result, "string", "ASCII should return string");
      assertEquals(result, testContent, "ASCII content should match");
      await fileHandle.close();

      // Test Base64 encoding
      fileHandle = await fs.open(tempFile, "r");
      result = await fileHandle.readFile({ encoding: "base64" });
      assertEquals(typeof result, "string", "Base64 should return string");
      // Verify it's valid base64 by decoding
      const decoded = Buffer.from(result as string, "base64").toString("utf8");
      assertEquals(decoded, testContent, "Decoded base64 should match");
      await fileHandle.close();

      // Test with explicit null encoding (should return Buffer)
      fileHandle = await fs.open(tempFile, "r");
      result = await fileHandle.readFile({ encoding: null });
      assert(result instanceof Buffer, "Null encoding should return Buffer");
      await fileHandle.close();
    } finally {
      await Deno.remove(tempFile);
    }
  },
});

/**
 * Test Case 8: Verify handle is NOT closed after readFile
 */
Deno.test({
  name: "FileHandle.readFile() - Does not close handle",
  async fn() {
    const tempFile = await Deno.makeTempFile();
    await Deno.writeTextFile(tempFile, testContent);

    try {
      const fileHandle = await fs.open(tempFile, "r");

      // Read the file
      await fileHandle.readFile();

      // Verify we can perform multiple operations
      const stat1 = await fileHandle.stat();
      assert(stat1.isFile(), "First stat should work");

      const stat2 = await fileHandle.stat();
      assert(stat2.isFile(), "Second stat should work");

      // Verify we can explicitly close it
      await fileHandle.close();

      // After closing, operations should fail
      let errorThrown = false;
      try {
        await fileHandle.stat();
      } catch (_error) {
        errorThrown = true;
      }
      assert(errorThrown, "Operations should fail after explicit close");
    } finally {
      await Deno.remove(tempFile);
    }
  },
});
