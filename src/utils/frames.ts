export type Frame = {
  size: number;
  index: number;
  data: Uint8Array;
};

/* to encrypt a file we use first the createFixedSizeFramesStream, then encrypt each frame
   using createFrameMapperStream and then join the frames with the size info using
   createVariableSizeFrameJoinStream

   but when decrypting we first extract the frames using createVariableSizeFramesStream, then
   decrypt each frame using createFrameMapperStream and finally join them all together without
   the block size bytes using createFrameJoinStream */

export function createFixedSizeFramesStream(chunkSize: number) {
  let buffer = new Uint8Array(0);
  let index = 0;
  return new TransformStream<Uint8Array, Frame>({
    async transform(chunk, controller) {
      const merged = new Uint8Array(buffer.length + chunk.length);
      merged.set(buffer);
      merged.set(chunk, buffer.length);
      buffer = merged;

      while (buffer.length >= chunkSize) {
        const frame = {
          size: chunkSize,
          index,
          data: buffer.slice(0, chunkSize),
        };
        controller.enqueue(frame);
        index++;
        buffer = buffer.slice(chunkSize);
      }
    },
    async flush(controller) {
      if (buffer.length > 0) {
        const frame = {
          size: buffer.length,
          index,
          data: buffer,
        };
        controller.enqueue(frame);
        index++;
      }
    },
  });
}

export function numberToByteArray(size: number): Uint8Array {
  let pos = 3; // start on 4th byte
  let rem = size; // begin with full size value

  const bytes = new Uint8Array(4);

  while (rem > 0 && pos >= 0) {
    const byteValue = rem % 256;
    bytes[pos] = byteValue;
    rem = (rem - byteValue) / 256;
    pos = pos - 1;
  }

  return bytes;
}

export function byteArrayToNumber(bytes: Uint8Array) {
  let pos = 0;
  let value = 0;

  while (pos < 4) {
    value = value * 256;
    value = value + bytes[pos];
    pos++;
  }

  return value;
}

export function createVariableSizeFramesStream() {
  let buffer = new Uint8Array(0);
  let index = 0;
  return new TransformStream<Uint8Array, Frame>({
    async transform(chunk, controller) {
      const merged = new Uint8Array(buffer.length + chunk.length);
      merged.set(buffer);
      merged.set(chunk, buffer.length);
      buffer = merged;

      while (true) {
        if (buffer.length < 4) {
          break;
        }

        const chunkSize = byteArrayToNumber(buffer.slice(0, 4));

        if (buffer.length < 4 + chunkSize) {
          break;
        }

        const frame = {
          size: chunkSize,
          index,
          data: buffer.slice(4, chunkSize + 4),
        };

        controller.enqueue(frame);

        index++;
        buffer = buffer.slice(4 + chunkSize);
      }
    },
  });
}

export function createFrameMapperStream(
  transformFn: (chunk: Frame) => Promise<Frame>,
) {
  return new TransformStream<Frame, Frame>({
    async transform(chunk, controller) {
      const newChunk = await transformFn(chunk);
      controller.enqueue(newChunk);
    },
  });
}

export function createFrameJoinStream() {
  return new TransformStream<Frame, Uint8Array>({
    async transform(chunk, controller) {
      controller.enqueue(chunk.data);
    },
  });
}

export function createVariableSizeFrameJoinStream() {
  return new TransformStream<Frame, Uint8Array>({
    async transform(chunk, controller) {
      const value = new Uint8Array(chunk.size + 4);
      value.set(numberToByteArray(chunk.size));
      value.set(chunk.data, 4);
      controller.enqueue(value);
    },
  });
}
