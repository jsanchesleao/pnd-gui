# Pnd-GUI

Based on my paranoid cli, this utility uses the Web Crypto API to encrypt and decrypt local files. Encrypted files are saved with a `.lock` extension appended to their original filename (e.g. `photo.jpg` → `photo.jpg.lock`).

## Encrypt / Decrypt

1. Open the app and select the **Encrypt / Decrypt** tab.
2. Click **Choose file** and select the file you want to encrypt or decrypt.
3. Choose the operation:
   - **Encrypt** — enter a password and click **Encrypt**. You will be prompted to choose where to save the output file (original filename + `.lock`).
   - **Decrypt** — enter the password used during encryption and click **Decrypt**. You will be prompted to choose where to save the decrypted file (`.lock` extension removed).
4. A progress bar is shown while the file is being processed.

> Encryption uses AES-256-GCM with a PBKDF2-derived key (100,000 iterations, SHA-256). Each file gets a unique random salt and IV.

## Preview

The **Preview** tab lets you view encrypted media files without saving the decrypted content to disk.

1. Open the app and select the **Preview** tab.
2. Click **Choose encrypted file** and select a `.lock` file.
3. The correct viewer is chosen automatically based on the file type:

| Original extension | Viewer |
|--------------------|--------|
| `.jpg`, `.jpeg`, `.png`, `.gif`, `.webp`, `.avif`, `.bmp`, `.svg` | Image viewer |
| `.mp4`, `.webm`, `.mkv`, `.mov`, `.avi` | Video player |
| `.zip` | Image gallery (carousel of all images inside the archive) |

4. Enter the password and press **Enter** or click **View** / **Play**.
5. The file is decrypted in memory and displayed immediately.

### Gallery navigation

When previewing a `.zip.lock` file, all images inside the archive are extracted and shown as a carousel:

- Click the **‹** / **›** buttons, or press the **left / right arrow keys**, to move between images.
- The current position is shown as `n / total` and the filename is displayed below the image.
- Click **Close** to return to the file picker.
