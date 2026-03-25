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

## Vault

A vault is an **encrypted file container** stored as a regular folder on disk. All files inside are individually encrypted and the folder can be safely synced or backed up — nothing is readable without the master password.

### Creating a vault

1. Open the app and select the **Vault** tab.
2. Click **New Vault** and choose an empty folder to use as the vault.
3. Enter a master password and click **Create**. An `index.lock` file is written to the folder and the vault opens.

### Opening an existing vault

1. Click **Open Vault** and select the vault folder.
2. Enter the master password and click **Unlock**.

### Managing files

Once the vault is open a two-panel browser is shown:

- **Left panel** — folder tree for navigating virtual folders inside the vault.
- **Right panel** — files in the current folder with their sizes.

Available actions from the toolbar:

| Action | Description |
|--------|-------------|
| **+ Add Files** | Select one or more files to encrypt and add to the current folder. A progress bar is shown while files are being processed. |
| **+ New Folder** | Create a virtual subfolder inside the vault. |
| **Save** | Persist all changes back to disk. The button shows a dot indicator when there are unsaved changes. |
| **Close** | Close the vault. You will be warned if there are unsaved changes. |

Per-file actions (accessible from each file row):

| Action | Description |
|--------|-------------|
| **Preview** | Decrypt the file in memory and display it without saving anything to disk (same viewer logic as the Preview tab). |
| **Export** | Decrypt and save the file to a location you choose. |
| **Rename** | Rename the file inside the vault. |
| **Move** | Move the file to a different virtual folder. |
| **Delete** | Remove the file from the vault and delete its encrypted data from disk. |

> Changes to files (add, delete, rename, move) are held in memory until you click **Save**.

### How it works

A vault folder contains:

- **`index.lock`** — an encrypted JSON index that maps every stored file to its metadata (name, virtual path, size) and the encryption keys for its data blocks. The index itself is encrypted with the master password using AES-256-GCM / PBKDF2.
- **UUID-named data files** — each stored file is split into blocks of up to 256 MB. Every block is encrypted with its own randomly generated AES-256-GCM key. That key is stored inside `index.lock`.

Because each block has a unique key and the index is the only place those keys live, an attacker who obtains the vault folder cannot read any file without first decrypting `index.lock` with the master password.
