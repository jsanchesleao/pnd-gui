export const fsaSupported =
  typeof window !== "undefined" && "showOpenFilePicker" in window;

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

export async function pickFiles(): Promise<File[]> {
  if (fsaSupported) {
    const handles = await window.showOpenFilePicker({
      multiple: true,
    } as Parameters<typeof window.showOpenFilePicker>[0]);
    return Promise.all(handles.map((h) => h.getFile()));
  }
  return promptFileInput(true);
}
