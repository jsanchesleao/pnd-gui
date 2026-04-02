/**
 * Extracts a single frame from video bytes and returns it as a WebP Uint8Array.
 * Returns null if extraction fails (unsupported codec, video too short, etc.).
 *
 * Only the bytes passed in are used — for large multi-part vault files, pass
 * only the first decrypted part to keep memory usage bounded.
 */
export async function generateVideoThumbnail(
  videoBytes: Uint8Array,
  mimeType: string,
  seekSeconds = 2,
  outputWidth = 260,
  quality = 0.7,
): Promise<Uint8Array | null> {
  const blob = new Blob([videoBytes], { type: mimeType });
  const url = URL.createObjectURL(blob);

  try {
    return await new Promise<Uint8Array | null>((resolve) => {
      const video = document.createElement("video");
      video.muted = true;
      video.playsInline = true;
      video.preload = "metadata";

      let settled = false;

      const timeout = setTimeout(() => {
        if (settled) return;
        settled = true;
        cleanup();
        resolve(null);
      }, 15_000);

      function cleanup() {
        clearTimeout(timeout);
        URL.revokeObjectURL(url);
        video.src = "";
        video.load();
      }

      function fail() {
        if (settled) return;
        settled = true;
        cleanup();
        resolve(null);
      }

      video.addEventListener("error", fail, { once: true });

      video.addEventListener(
        "loadedmetadata",
        () => {
          const target =
            isFinite(video.duration) && video.duration > 0
              ? Math.min(seekSeconds, video.duration * 0.1)
              : 0;
          video.currentTime = target;
        },
        { once: true },
      );

      video.addEventListener(
        "seeked",
        () => {
          if (settled) return;
          settled = true;
          try {
            const aspectRatio =
              video.videoHeight > 0
                ? video.videoWidth / video.videoHeight
                : 16 / 9;
            const canvas = document.createElement("canvas");
            canvas.width = outputWidth;
            canvas.height = Math.round(outputWidth / aspectRatio);
            const ctx = canvas.getContext("2d");
            if (!ctx) {
              cleanup();
              resolve(null);
              return;
            }
            ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
            canvas.toBlob(
              (canvasBlob) => {
                cleanup();
                if (!canvasBlob) {
                  resolve(null);
                  return;
                }
                canvasBlob
                  .arrayBuffer()
                  .then((ab) => resolve(new Uint8Array(ab)));
              },
              "image/webp",
              quality,
            );
          } catch {
            cleanup();
            resolve(null);
          }
        },
        { once: true },
      );

      video.src = url;
    });
  } catch {
    URL.revokeObjectURL(url);
    return null;
  }
}
