# Vault — Preview and Export

## Preview a vault entry

### Synopsis

```
pnd-cli --vault-preview <vault-path> [<vault-dir>]
pnd-cli --vault-preview <vault-path> [--vault-dir <DIR>]
```

### Description

Non-interactive. Decrypts the entry at `<vault-path>` entirely into memory and opens the
same viewer pipeline as `pnd-cli -p` (Kitty inline image, mpv, bat, or built-in text
viewer). The decrypted bytes are never written to disk.

`<vault-path>` is the full virtual path of the entry inside the vault, for example:
- `notes.txt` — a file at the vault root
- `photos/summer/beach.jpg` — a file in a nested folder

`<vault-dir>` defaults to the current directory when omitted.

### Examples

```bash
# Preview an image from the vault
pnd-cli --vault-preview photos/summer/beach.jpg

# Preview from a specific vault
pnd-cli --vault-preview notes.txt ~/vaults/personal

# Equivalent using --vault-dir
pnd-cli --vault-preview notes.txt --vault-dir ~/vaults/personal
```

### Edge cases

| Situation | Behaviour |
|---|---|
| `<vault-path>` not found in index | stderr error, exit 2 |
| Blob file missing from disk | stderr error (corrupted vault), exit 2 |
| Wrong password | Exit 1 |
| Unsupported file type | "No previewer for `.<ext>` files"; exit 0 |
| mpv not installed (media file) | Install hint on stderr; exit 2 |

---

## Export a vault entry to disk

### Synopsis

```
pnd-cli --vault-export <vault-path> [OPTIONS] [<vault-dir>]
```

### Description

Non-interactive. Decrypts the file (or all files in a folder) at `<vault-path>` and
writes the plaintext to `--dest` (defaults to the current directory).

`<vault-path>` can be either:
- **A file path** — exports that single file. The output filename is taken from the
  vault entry's `name` field.
- **A folder path** — exports all files directly inside that folder. With `-r`, exports
  all files in the folder and every subfolder recursively, preserving the relative
  sub-path structure under `--dest`.

When exporting a folder, a confirmation prompt is shown (`Extract N file(s) into <dir>?
[y/N]`) unless `-y` is given or stdin is not a TTY.

All output is written atomically (temp file → rename). A pre-flight collision check is
performed for folder exports before any file is written; if any destination file already
exists and `--force` is not given, the entire export is aborted.

### Options

| Flag | Description |
|---|---|
| `--dest <DIR>` | Destination directory (default: current directory) |
| `-f`, `--force` | Overwrite destination files that already exist |
| `-r`, `--recursive` | Include files in subfolders when exporting a folder path |
| `-y`, `--yes` | Skip the confirmation prompt for folder exports |
| `--vault-dir <DIR>` | Vault directory (alternative to positional argument) |

### Examples

```bash
# Export a single file to the current directory
pnd-cli --vault-export photos/summer/beach.jpg

# Export to a specific directory
pnd-cli --vault-export documents/report.pdf --dest ~/Downloads

# Export all files in the "photos" folder
pnd-cli --vault-export photos --dest ~/exported-photos

# Export the entire vault (recursive from root)
pnd-cli --vault-export "" --dest ~/vault-backup -r -y

# Overwrite existing files
pnd-cli --vault-export notes.txt --force
```

### Edge cases

| Situation | Behaviour |
|---|---|
| `<vault-path>` not found (not a file or folder) | stderr error, exit 2 |
| `--dest` directory does not exist | stderr error, exit 2; directory not created |
| Destination file already exists, `--force` not given | stderr error, exit 4 |
| Blob file missing | stderr error (corrupted vault), exit 2 |
| Wrong password | Exit 1 |
| Folder export: stdin not a TTY, `-y` not given | Confirmation prompt is skipped (non-interactive assumption) |

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Wrong password / authentication failure |
| `2` | I/O error, not found, or corrupted vault |
| `3` | Bad arguments |
| `4` | Destination file(s) exist and `--force` not given |
