# Vault — List Contents

## Synopsis

```
pnd-cli --vault-list [<vault-dir>]
pnd-cli --vault-list [--vault-dir <DIR>]
```

## Description

Non-interactive. Decrypts `index.lock`, then prints all vault entries to stdout. No blob
files are read.

`<vault-dir>` defaults to the **current directory** when omitted. It can also be supplied
via the global `--vault-dir` flag instead of as a positional argument.

## Options

| Flag | Description |
|---|---|
| `--json` | Print entries as a JSON array instead of human-readable text |
| `--path <VAULT_PATH>` | List only entries under this virtual folder (and its subfolders) |
| `--vault-dir <DIR>` | Vault directory (alternative to positional argument) |

## Output formats

### Human-readable (default)

One entry per line. Entries are aligned in two columns: full virtual path and size.

```
photos/summer/beach.jpg      (3.1 MB)
documents/report.pdf         (128.0 KB)
notes.txt                    (4.0 KB)
```

### JSON (`--json`)

A JSON array of objects. Each object has three fields:

```json
[
  {"path": "photos/summer/beach.jpg", "name": "beach.jpg", "size": 3251200},
  {"path": "documents/report.pdf",    "name": "report.pdf","size": 131072},
  {"path": "notes.txt",               "name": "notes.txt", "size": 4096}
]
```

`path` is the full virtual path (`folder/name` or just `name` at the root). `size` is
the original plaintext byte count.

## Examples

```bash
# List all entries in the vault at the current directory
pnd-cli --vault-list

# List entries in a specific vault
pnd-cli --vault-list ~/vaults/work

# List only entries under the "photos" folder
pnd-cli --vault-list --path photos

# Machine-readable output for scripting
pnd-cli --vault-list --json | jq '.[].path'

# Count entries
pnd-cli --vault-list --json | jq 'length'
```

## Edge cases

| Situation | Behaviour |
|---|---|
| `<vault-dir>` does not exist | stderr error, exit 2 |
| `<vault-dir>` is not a vault (no `index.lock`) | stderr error, exit 2 |
| `<vault-dir>` is a file, not a directory | stderr error, exit 3 |
| Wrong password | stderr error, exit 1 |
| Vault is empty | Prints nothing (or `[]` with `--json`); exit 0 |
| `--path` matches no entries | Prints nothing (or `[]` with `--json`); exit 0 |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Wrong password / authentication failure |
| `2` | I/O or filesystem error |
| `3` | Bad arguments |
