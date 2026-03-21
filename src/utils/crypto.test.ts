import { describe, expect, it } from "vitest";
import { createDecryptedStream, createEncryptedStream } from "./crypto";

function makeStream(data: Uint8Array): ReadableStream<Uint8Array> {
  return new ReadableStream({
    start(controller) {
      controller.enqueue(data);
      controller.close();
    },
  });
}

async function collectStream(
  stream: ReadableStream<Uint8Array>,
): Promise<Uint8Array> {
  const chunks: Uint8Array[] = [];
  const reader = stream.getReader();
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }
  const total = chunks.reduce((n, c) => n + c.length, 0);
  const result = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }
  return result;
}

async function roundtrip(
  data: Uint8Array,
  password: string,
  chunkSize?: number,
): Promise<Uint8Array> {
  const encrypted = createEncryptedStream(
    makeStream(data),
    password,
    chunkSize,
  );
  const decrypted = createDecryptedStream(encrypted, password);
  return collectStream(decrypted);
}

describe("createEncryptedStream + createDecryptedStream", () => {
  it("roundtrips small data (single chunk)", async () => {
    const data = new Uint8Array([1, 2, 3, 4, 5]);
    expect(await roundtrip(data, "password")).toEqual(data);
  });

  it("roundtrips data spanning multiple chunks", async () => {
    const data = new Uint8Array(300).map((_, i) => i % 256);
    expect(await roundtrip(data, "password", 100)).toEqual(data);
  });

  it("roundtrips empty input", async () => {
    expect(await roundtrip(new Uint8Array(0), "password")).toEqual(
      new Uint8Array(0),
    );
  });

  it("roundtrips data that is exactly one chunk", async () => {
    const data = new Uint8Array(256).fill(42);
    expect(await roundtrip(data, "password", 256)).toEqual(data);
  });

  it("roundtrips with a custom small chunk size", async () => {
    const data = new Uint8Array(1000).map((_, i) => i % 256);
    expect(await roundtrip(data, "password", 64)).toEqual(data);
  });

  it("encrypted output is larger than input due to per-frame overhead", async () => {
    const data = new Uint8Array(100).fill(1);
    const encrypted = await collectStream(
      createEncryptedStream(makeStream(data), "password"),
    );
    // Each frame adds: 4-byte size prefix + 16-byte salt + 12-byte IV + 16-byte GCM tag = 48 bytes overhead
    expect(encrypted.length).toBeGreaterThan(data.length);
  });

  it("produces different ciphertext each time for the same input", async () => {
    const data = new Uint8Array([1, 2, 3]);
    const first = await collectStream(
      createEncryptedStream(makeStream(data), "password"),
    );
    const second = await collectStream(
      createEncryptedStream(makeStream(data), "password"),
    );
    expect(first).not.toEqual(second);
  });
});

describe("createDecryptedStream error handling", () => {
  it("throws when decrypting with the wrong password", async () => {
    const data = new Uint8Array([10, 20, 30]);
    const encrypted = createEncryptedStream(makeStream(data), "correct");
    const decrypted = createDecryptedStream(encrypted, "wrong");

    await expect(collectStream(decrypted)).rejects.toThrow();
  });
});
