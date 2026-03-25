import {
  createFixedSizeFramesStream,
  createFrameJoinStream,
  createFrameMapperStream,
  createVariableSizeFrameJoinStream,
  createVariableSizeFramesStream,
} from "./frames";

async function deriveKey(password: string, salt: ArrayBuffer) {
  const encoder = new TextEncoder();
  const passwordKey = await crypto.subtle.importKey(
    "raw",
    encoder.encode(password),
    { name: "PBKDF2" },
    false,
    ["deriveKey"],
  );
  const key = await crypto.subtle.deriveKey(
    { name: "PBKDF2", salt, iterations: 100000, hash: "SHA-256" },
    passwordKey,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );

  return key;
}

export async function encryptBytes(bytes: Uint8Array, password: string) {
  const salt = createSalt();
  const iv = createIV();
  const key = await deriveKey(password, salt);

  const encryptedContent = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv },
    key,
    bytes,
  );

  return new Blob([salt, iv, new Uint8Array(encryptedContent)]);
}

export async function decryptBytes(encryptedBlob: Blob, password: string) {
  const data = await encryptedBlob.arrayBuffer();
  const salt = data.slice(0, 16);
  const iv = data.slice(16, 28);
  const cypherText = data.slice(28);

  const key = await deriveKey(password, salt);
  try {
    const decryptedContent = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv },
      key,
      cypherText,
    );
    return new Uint8Array(decryptedContent);
  } catch (e) {
    return null;
  }
}

export function createEncryptedStream(
  stream: ReadableStream<Uint8Array>,
  password: string,
  chunkSize: number = 1024 * 1024,
) {
  return stream
    .pipeThrough(createFixedSizeFramesStream(chunkSize))
    .pipeThrough(
      createFrameMapperStream(async (frame) => {
        const encryptedBlob = await encryptBytes(frame.data, password);
        const encryptedBytes = new Uint8Array(
          await encryptedBlob.arrayBuffer(),
        );
        return {
          size: encryptedBytes.length,
          index: frame.index,
          data: encryptedBytes,
        };
      }),
    )
    .pipeThrough(createVariableSizeFrameJoinStream());
}

export function createDecryptedStream(
  stream: ReadableStream<Uint8Array>,
  password: string,
) {
  return stream
    .pipeThrough(createVariableSizeFramesStream())
    .pipeThrough(
      createFrameMapperStream(async (frame) => {
        const decryptedBytes = await decryptBytes(
          new Blob([frame.data]),
          password,
        );
        if (decryptedBytes === null)
          throw new Error(`Failed to decrypt frame ${frame.index}`);
        return {
          size: decryptedBytes.length,
          index: frame.index,
          data: decryptedBytes,
        };
      }),
    )
    .pipeThrough(createFrameJoinStream());
}

function createSalt(): ArrayBuffer {
  return crypto.getRandomValues(new Uint8Array(16));
}

function createIV(): ArrayBuffer {
  return crypto.getRandomValues(new Uint8Array(12));
}

export async function generateFileKey(): Promise<CryptoKey> {
  return crypto.subtle.generateKey(
    { name: "AES-GCM", length: 256 },
    true,
    ["encrypt", "decrypt"],
  );
}

export async function encryptBytesWithKey(
  bytes: Uint8Array,
  key: CryptoKey,
): Promise<Blob> {
  const salt = createSalt();
  const iv = createIV();
  const encryptedContent = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv },
    key,
    bytes,
  );
  return new Blob([salt, iv, new Uint8Array(encryptedContent)]);
}

export async function decryptBytesWithKey(
  encryptedBlob: Blob,
  key: CryptoKey,
): Promise<Uint8Array | null> {
  const data = await encryptedBlob.arrayBuffer();
  const iv = data.slice(16, 28);
  const cypherText = data.slice(28);
  try {
    const decryptedContent = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv },
      key,
      cypherText,
    );
    return new Uint8Array(decryptedContent);
  } catch {
    return null;
  }
}

export async function exportKeyToBase64(key: CryptoKey): Promise<string> {
  const raw = await crypto.subtle.exportKey("raw", key);
  return btoa(String.fromCharCode(...new Uint8Array(raw)));
}

export async function importKeyFromBase64(b64: string): Promise<CryptoKey> {
  const raw = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  return crypto.subtle.importKey(
    "raw",
    raw,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}
