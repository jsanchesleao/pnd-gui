# Vault — Add Files

## Synopsis

```
pnd-cli --vault-add <file>... [OPTIONS]
```

## Description

Non-interactive. Encrypts one or more local files and adds them to the vault index.

Each file is read, split into 256 MiB blocks if necessary, each block encrypted with a
fresh random AES-256 key, and written to the vault as a UUID-named blob file. The vault
index (`index.lock`) is updated and saved atomically after each successful add.

**Multi-file behaviour:** files are processed in order. If one file fails (not found,
collision, I/O error), that file and all remaining files are skipped, but files
successfully added before the failure are kept in the vault. The overall exit code is 2
if any file failed.

### Naming and placement

By default, files are placed in the vault root (`""`). Use `--vault-path` to put them in
a virtual subfolder (which is created implicitly — no separate folder-creation step is
needed).

The filename used inside the vault is taken from the local file's name. Vault path
components are normalised: leading and trailing slashes are stripped silently.

### Collision handling

If a file with the same name already exists at `--vault-path`:
- Without `--force`: an error is printed and the file is skipped (exit 4).
- With `--force`: the existing entry and its blob files are removed before adding the
  new version.

## Options

| Flag | Description |
|---|---|
| `--vault-path <PATH>` | Virtual folder inside the vault where files are placed (default: root `""`) |
| `--vault-dir <DIR>` | Vault directory (default: current directory) |
| `-f`, `--force` | Replace an existing file with the same name at `--vault-path` |

## Examples

```bash
# Add a single file to the vault root
pnd-cli --vault-add photo.jpg

# Add a file to a nested virtual folder (created implicitly)
pnd-cli --vault-add report.pdf --vault-path documents/2024

# Add multiple files at once
pnd-cli --vault-add *.jpg --vault-path photos/summer

# Add to a specific vault
pnd-cli --vault-add secrets.txt --vault-dir ~/vaults/work

# Replace an existing file
pnd-cli --vault-add updated-report.pdf --vault-path documents --force
```

## Edge cases

| Situation | Behaviour |
|---|---|
| `<file>` does not exist | Error for that file; skip it; continue with the rest; exit 2 at end |
| `<file>` is a directory | Error for that file; skip it; exit 3 at end |
| Name collision, `--force` not given | Error; skip that file; exit 4 at end |
| Name collision, `--force` given | Old entry and blobs removed; new version added |
| Disk full while writing blob | Partial blob deleted; index not updated for that file; exit 2 |
| Wrong password | Exit 1 (no files are added) |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | All files added successfully |
| `1` | Wrong password / authentication failure |
| `2` | One or more files could not be added (I/O error or not found) |
| `3` | One or more files skipped because they are directories |
| `4` | One or more name collisions when `--force` not given |
