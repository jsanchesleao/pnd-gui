# Vault — Add Files

## Synopsis

```
pnd-cli --vault-add <file>... [OPTIONS]
pnd-cli --vault-add - --name <NAME> [OPTIONS]   # read from stdin
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

### Reading from stdin

Pass `-` as the sole file argument to read from standard input. Because there is no
filename to derive the vault entry name from, `--name` is required:

```bash
date | PND_PASSWORD=pw pnd-cli --vault-add - --name timestamp.txt
```

Mixing `-` with real file paths in a single invocation is an error (exit 3). The entry
size stored in the index is set to the number of bytes read from stdin.

### Naming and placement

By default, files are placed in the vault root (`""`). Use `--vault-path` to put them in
a virtual subfolder (which is created implicitly — no separate folder-creation step is
needed).

The filename used inside the vault is taken from the local file's name (or `--name` for
stdin). Vault path components are normalised: leading and trailing slashes are stripped
silently.

### Collision handling

If a file with the same name already exists at `--vault-path`:
- Without `--force`: an error is printed and the file is skipped (exit 2).
- With `--force`: the existing entry and its blob files are removed before adding the
  new version.

## Options

| Flag | Description |
|---|---|
| `--vault-path <PATH>` | Virtual folder inside the vault where files are placed (default: root `""`) |
| `--vault-dir <DIR>` | Vault directory (default: current directory) |
| `--name <NAME>` | Vault entry name — required when the source is stdin (`-`) |
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

# Add from stdin (name must be supplied explicitly)
echo "my note" | PND_PASSWORD=pw pnd-cli --vault-add - --name note.txt

# Add generated content to a subfolder
date | PND_PASSWORD=pw pnd-cli --vault-add - --name timestamp.txt --vault-path logs

# Combine with vault-export --stdout to copy an entry through a pipe
PND_PASSWORD=pw pnd-cli --vault-export report.pdf --stdout \
  | PND_PASSWORD=pw pnd-cli --vault-add - --name report.pdf --vault-dir ~/vaults/backup
```

## Edge cases

| Situation | Behaviour |
|---|---|
| `<file>` does not exist | Error for that file; skip it; continue with the rest; exit 2 at end |
| `<file>` is a directory | Error for that file; skip it; exit 2 at end |
| `-` mixed with real file paths | stderr error, exit 3 |
| `-` without `--name` | stderr error, exit 3 |
| Name collision, `--force` not given | Error; exit 2 |
| Name collision, `--force` given | Old entry and blobs removed; new version added |
| Disk full while writing blob | Partial blob deleted; index not updated for that file; exit 2 |
| Wrong password | Exit 1 (no files are added) |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | All files added successfully |
| `1` | Wrong password / authentication failure |
| `2` | One or more files could not be added (I/O error, not found, or collision) |
| `3` | Bad arguments (stdin source mixed with files, or `--name` missing) |
