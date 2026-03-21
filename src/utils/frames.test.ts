import { describe, expect, it } from "vitest";
import {
  byteArrayToNumber,
  createFixedSizeFramesStream,
  createFrameJoinStream,
  createFrameMapperStream,
  createVariableSizeFrameJoinStream,
  createVariableSizeFramesStream,
  numberToByteArray,
  type Frame,
} from "./frames";

async function collect<T>(stream: ReadableStream<T>): Promise<T[]> {
  const results: T[] = [];
  const reader = stream.getReader();
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    results.push(value);
  }
  return results;
}

function makeStream(data: Uint8Array): ReadableStream<Uint8Array> {
  return new ReadableStream({
    start(controller) {
      controller.enqueue(data);
      controller.close();
    },
  });
}

function makeFrameStream(frames: Frame[]): ReadableStream<Frame> {
  return new ReadableStream({
    start(controller) {
      for (const frame of frames) controller.enqueue(frame);
      controller.close();
    },
  });
}

describe("numberToByteArray / byteArrayToNumber", () => {
  it.each([0, 1, 255, 256, 65535, 16777215, 2 ** 32 - 1])(
    "roundtrips %i",
    (n) => {
      expect(byteArrayToNumber(numberToByteArray(n))).toBe(n);
    },
  );

  it("encodes 1 as [0, 0, 0, 1]", () => {
    expect(Array.from(numberToByteArray(1))).toEqual([0, 0, 0, 1]);
  });

  it("encodes 256 as [0, 0, 1, 0]", () => {
    expect(Array.from(numberToByteArray(256))).toEqual([0, 0, 1, 0]);
  });

  it("always returns a 4-byte array", () => {
    expect(numberToByteArray(0)).toHaveLength(4);
    expect(numberToByteArray(2 ** 32 - 1)).toHaveLength(4);
  });
});

describe("createFixedSizeFramesStream", () => {
  it("splits data into exact-size frames when input is a multiple of chunkSize", async () => {
    const data = new Uint8Array(9).map((_, i) => i);
    const frames = await collect(
      makeStream(data).pipeThrough(createFixedSizeFramesStream(3)),
    );

    expect(frames).toHaveLength(3);
    expect(frames[0].data).toEqual(new Uint8Array([0, 1, 2]));
    expect(frames[1].data).toEqual(new Uint8Array([3, 4, 5]));
    expect(frames[2].data).toEqual(new Uint8Array([6, 7, 8]));
  });

  it("emits a partial final frame when input is not a multiple of chunkSize", async () => {
    const data = new Uint8Array([0, 1, 2, 3, 4]);
    const frames = await collect(
      makeStream(data).pipeThrough(createFixedSizeFramesStream(3)),
    );

    expect(frames).toHaveLength(2);
    expect(frames[0].data).toEqual(new Uint8Array([0, 1, 2]));
    expect(frames[1].data).toEqual(new Uint8Array([3, 4]));
  });

  it("produces a single frame when input is smaller than chunkSize", async () => {
    const data = new Uint8Array([10, 20, 30]);
    const frames = await collect(
      makeStream(data).pipeThrough(createFixedSizeFramesStream(100)),
    );

    expect(frames).toHaveLength(1);
    expect(frames[0].data).toEqual(data);
  });

  it("assigns sequential indices starting from 0", async () => {
    const data = new Uint8Array(6).fill(1);
    const frames = await collect(
      makeStream(data).pipeThrough(createFixedSizeFramesStream(2)),
    );

    expect(frames.map((f) => f.index)).toEqual([0, 1, 2]);
  });

  it("sets frame size to match data length", async () => {
    const data = new Uint8Array([1, 2, 3, 4, 5]);
    const frames = await collect(
      makeStream(data).pipeThrough(createFixedSizeFramesStream(3)),
    );

    expect(frames[0].size).toBe(3);
    expect(frames[1].size).toBe(2);
  });
});

describe("createVariableSizeFrameJoinStream + createVariableSizeFramesStream", () => {
  it("roundtrips frames of varying sizes", async () => {
    const original: Frame[] = [
      { size: 3, index: 0, data: new Uint8Array([1, 2, 3]) },
      { size: 5, index: 1, data: new Uint8Array([4, 5, 6, 7, 8]) },
      { size: 1, index: 2, data: new Uint8Array([9]) },
    ];

    const encoded = makeFrameStream(original).pipeThrough(
      createVariableSizeFrameJoinStream(),
    );
    const decoded = await collect(
      encoded.pipeThrough(createVariableSizeFramesStream()),
    );

    expect(decoded).toHaveLength(3);
    decoded.forEach((frame, i) => {
      expect(frame.size).toBe(original[i].size);
      expect(frame.data).toEqual(original[i].data);
      expect(frame.index).toBe(i);
    });
  });

  it("prepends a 4-byte size prefix to each frame", async () => {
    const frame: Frame = { size: 2, index: 0, data: new Uint8Array([0xab, 0xcd]) };
    const chunks = await collect(
      makeFrameStream([frame]).pipeThrough(createVariableSizeFrameJoinStream()),
    );

    expect(chunks[0]).toHaveLength(6); // 4-byte prefix + 2 data bytes
    expect(Array.from(chunks[0].slice(0, 4))).toEqual([0, 0, 0, 2]);
    expect(Array.from(chunks[0].slice(4))).toEqual([0xab, 0xcd]);
  });
});

describe("createFrameMapperStream", () => {
  it("applies the transform to every frame", async () => {
    const frames: Frame[] = [
      { size: 2, index: 0, data: new Uint8Array([1, 2]) },
      { size: 2, index: 1, data: new Uint8Array([3, 4]) },
    ];
    const doubled = await collect(
      makeFrameStream(frames).pipeThrough(
        createFrameMapperStream(async (f) => ({
          ...f,
          data: f.data.map((b) => b * 2) as Uint8Array,
        })),
      ),
    );

    expect(doubled[0].data).toEqual(new Uint8Array([2, 4]));
    expect(doubled[1].data).toEqual(new Uint8Array([6, 8]));
  });

  it("preserves frame indices", async () => {
    const frames: Frame[] = [
      { size: 1, index: 0, data: new Uint8Array([0]) },
      { size: 1, index: 1, data: new Uint8Array([0]) },
      { size: 1, index: 2, data: new Uint8Array([0]) },
    ];
    const result = await collect(
      makeFrameStream(frames).pipeThrough(
        createFrameMapperStream(async (f) => f),
      ),
    );

    expect(result.map((f) => f.index)).toEqual([0, 1, 2]);
  });
});

describe("createFrameJoinStream", () => {
  it("outputs raw data bytes in order, stripping frame metadata", async () => {
    const frames: Frame[] = [
      { size: 3, index: 0, data: new Uint8Array([1, 2, 3]) },
      { size: 2, index: 1, data: new Uint8Array([4, 5]) },
    ];
    const chunks = await collect(
      makeFrameStream(frames).pipeThrough(createFrameJoinStream()),
    );

    expect(chunks[0]).toEqual(new Uint8Array([1, 2, 3]));
    expect(chunks[1]).toEqual(new Uint8Array([4, 5]));
  });
});
