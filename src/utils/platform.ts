export const fsaSupported =
  typeof window !== "undefined" && "showOpenFilePicker" in window;

export const tauriSupported =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function promptFileInput(multiple = false): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "*/*";
    input.multiple = multiple;
    input.onchange = () => resolve(Array.from(input.files ?? []));
    input.click();
  });
}

export async function pickFile(): Promise<File | null> {
  if (fsaSupported) {
    const [handle] = await window.showOpenFilePicker();
    return handle.getFile();
  }
  const files = await promptFileInput();
  return files[0] ?? null;
}

export async function pickFileWithHandle(): Promise<{
  file: File;
  handle: FileSystemFileHandle | null;
} | null> {
  if (fsaSupported) {
    const [handle] = await window.showOpenFilePicker();
    const file = await handle.getFile();
    return { file, handle };
  }
  const files = await promptFileInput();
  if (!files[0]) return null;
  return { file: files[0], handle: null };
}

export async function pickFiles(): Promise<File[]> {
  if (fsaSupported) {
    const handles = await window.showOpenFilePicker({
      multiple: true,
    } as Parameters<typeof window.showOpenFilePicker>[0]);
    return Promise.all(handles.map((h) => h.getFile()));
  }
  return promptFileInput(true);
}
