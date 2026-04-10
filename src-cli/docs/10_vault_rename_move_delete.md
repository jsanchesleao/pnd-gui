# Vault — Rename, Move, and Delete

All three operations are **pure index mutations**: they decrypt `index.lock`, update
entries in memory, and save the index atomically (temp file → rename). No blob files are
read or written (except delete, which also removes blob files from disk). This makes
rename and move fast regardless of how large the files are.

---

## Rename a vault entry

### Synopsis

```
pnd-cli --vault-rename <vault-path> <new-name> [<vault-dir>]
pnd-cli --vault-rename <vault-path> <new-name> [--vault-dir <DIR>]
```

### Description

Renames the entry at `<vault-path>` within the vault. Only the `name` field in the index
is changed — the entry stays in its current virtual folder. The virtual path of the entry
becomes `<parent-folder>/<new-name>`.

`<new-name>` must be a bare filename with no `/` characters. To move an entry to a
different folder, use `--vault-move`.

### Options

| Flag | Description |
|---|---|
| `--vault-dir <DIR>` | Vault directory (default: current directory) |

### Examples

```bash
# Rename a file at the vault root
pnd-cli --vault-rename old-name.txt new-name.txt

# Rename a file in a nested folder
pnd-cli --vault-rename photos/summer/img001.jpg beach.jpg

# Rename in a specific vault
pnd-cli --vault-rename notes.txt journal.txt --vault-dir ~/vaults/personal
```

### Edge cases

| Situation | Behaviour |
|---|---|
| `<vault-path>` not found | stderr error, exit 2 |
| `<new-name>` already exists in the same folder | stderr error, exit 4 |
| `<new-name>` contains a `/` | stderr error with hint to use `--vault-move`, exit 3 |
| Renaming to the same name | "nothing to do" confirmation; exit 0 |
| Wrong password | Exit 1 |

---

## Move a vault entry

### Synopsis

```
pnd-cli --vault-move <vault-path> <dest-folder> [OPTIONS] [<vault-dir>]
```

### Description

Moves the entry at `<vault-path>` to a different virtual folder (`<dest-folder>`). Only
the path prefix in the index is changed — the filename is preserved unless `--name` is
also given.

`<dest-folder>` is a virtual path inside the vault (`photos/summer`, `""` for root).
Leading and trailing slashes are normalised silently.

Use `--name` to rename the entry at the same time as moving it, combining both operations
into a single index write.

### Options

| Flag | Description |
|---|---|
| `--name <NEW_NAME>` | Rename the entry while moving it (must not contain `/`) |
| `--vault-dir <DIR>` | Vault directory (default: current directory) |

### Examples

```bash
# Move a file to a different folder
pnd-cli --vault-move photos/beach.jpg archive

# Move to the vault root
pnd-cli --vault-move archive/old-report.pdf ""

# Move and rename in one step
pnd-cli --vault-move drafts/v1.txt final --name report.txt

# Move within a specific vault
pnd-cli --vault-move notes.txt journal --vault-dir ~/vaults/work
```

### Edge cases

| Situation | Behaviour |
|---|---|
| `<vault-path>` not found | stderr error, exit 2 |
| A file with the same name already exists at `<dest-folder>` | stderr error, exit 4 |
| Same folder and same name (no-op) | "nothing to do" confirmation; exit 0 |
| `--name` contains a `/` | stderr error, exit 3 |
| Wrong password | Exit 1 |

---

## Delete vault entries

### Synopsis

```
pnd-cli --vault-delete <vault-path>... [OPTIONS] [<vault-dir>]
```

### Description

Deletes one or more entries from the vault. Both the index entry and the corresponding
blob files are removed from disk. The index is saved **once** after all deletions are
complete.

### Confirmation

When stdin is a TTY and `-y` is not given, a confirmation prompt is shown before any
deletion takes place:

```
Delete 3 item(s)? [y/N]
```

Type `y` (case-insensitive) to confirm; anything else aborts with "Aborted." and exit 0.

When stdin is **not** a TTY (piped or redirected) and `-y` is not given, the command
prints an error and exits with code 3. This prevents silent data loss in scripts that
forget to add `-y`.

### Missing blobs

If a blob file is missing from disk for a matched entry (corrupted vault), a warning is
printed but the index entry is still removed. This leaves the vault in a self-consistent
state even if blob files were deleted externally.

### Multi-path behaviour

All requested paths are resolved before any deletion begins. Paths not found in the index
produce a warning and are skipped; the remaining valid paths are still deleted. The exit
code is 2 at the end if any path was not found.

### Options

| Flag | Description |
|---|---|
| `-y`, `--yes` | Skip the confirmation prompt |
| `--vault-dir <DIR>` | Vault directory (default: current directory) |

### Examples

```bash
# Delete a single file (prompts for confirmation on a TTY)
pnd-cli --vault-delete photos/old-photo.jpg

# Delete multiple files at once
pnd-cli --vault-delete drafts/v1.txt drafts/v2.txt

# Delete without confirmation (for scripts)
pnd-cli --vault-delete temp-file.txt -y

# Delete from a specific vault
pnd-cli --vault-delete secret.txt --vault-dir ~/vaults/work -y
```

### Edge cases

| Situation | Behaviour |
|---|---|
| `<vault-path>` not found | Warning printed; skip that entry; continue with rest; exit 2 at end |
| Blob file missing from disk | Index entry removed anyway; warning printed; continue |
| Multiple paths, one fails | All valid paths still deleted; exit 2 at end |
| `-y` not given and stdin is not a TTY | stderr error, exit 3 |
| User declines confirmation | "Aborted."; exit 0 |
| Wrong password | Exit 1 |

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Success (or user aborted confirmation) |
| `1` | Wrong password / authentication failure |
| `2` | One or more paths not found in the vault index |
| `3` | stdin is not a TTY and `-y` was not given |
