# Encrypt / Decrypt a Single File

## Synopsis

```
pnd-cli [OPTIONS] <file>
pnd-cli -m <encrypt|decrypt> [OPTIONS]       # read from stdin
pnd-cli -m <encrypt|decrypt> [OPTIONS] -     # explicit stdin
```

## Description

Encrypts or decrypts a single file. The mode is detected automatically from the file
extension:

- **`.lock` extension** → decrypt. Output is `<file>` with the `.lock` suffix removed.
- **Any other extension** → encrypt. Output is `<file>.lock`.

When reading from **stdin** (no file argument, or `-` as the file) the mode cannot be
inferred from a filename, so `--mode` / `-m` is required.

The password is read from the terminal (hidden) or from the `PND_PASSWORD` environment
variable. See [01_overview.md](01_overview.md) for password input details.

Output is written atomically: a temporary file is created in the same directory as the
output path, then renamed into place on success. If the operation fails at any point the
temporary file is deleted and the destination path is left untouched.

Progress is reported on stderr when stderr is connected to a terminal. The line is
overwritten in place (`\r`) so it does not scroll. When stderr is redirected or piped,
progress output is suppressed entirely. When reading from stdin, the file size is unknown
so a running byte count is shown instead of a percentage.

## Options

| Flag | Description |
|---|---|
| `-m`, `--mode <encrypt\|decrypt>` | Explicit operation mode — required when reading from stdin |
| `-o <PATH>` | Write output to `PATH` instead of the default location |
| `-c`, `--stdout` | Write output to stdout instead of a file |
| `-f`, `--force` | Overwrite the output file if it already exists |
| `-t`, `--tui` | Open the TUI Encrypt/Decrypt screen with `<file>` pre-loaded |

`--mode` accepts `encrypt` / `enc` / `e` and `decrypt` / `dec` / `d`.

## Output routing

| Source | `-o` given | `--stdout` given | Output destination |
|---|---|---|---|
| File | No | No | Default path (`<file>.lock` or `<file>`) |
| File | Yes | No | `-o PATH` |
| File | — | Yes | stdout |
| stdin | No | No | stdout (implicit) |
| stdin | Yes | No | `-o PATH` |
| stdin | — | Yes | stdout |

When `--stdout` and `-o` are both given, `--stdout` wins and a warning is printed.

## Examples

```bash
# Encrypt a file (produces report.pdf.lock)
pnd-cli report.pdf

# Decrypt a file (produces report.pdf)
pnd-cli report.pdf.lock

# Encrypt and write to a custom path
pnd-cli report.pdf -o /backups/report.enc

# Write decrypted output to stdout (pipe to another tool)
pnd-cli report.pdf.lock --stdout | grep "keyword"

# Encrypt from stdin, output to stdout
cat report.pdf | PND_PASSWORD=s3cr3t pnd-cli -m encrypt --stdout > report.pdf.lock

# Decrypt from stdin, output to stdout
cat report.pdf.lock | PND_PASSWORD=s3cr3t pnd-cli -m decrypt --stdout

# Chain: decrypt then re-encrypt with a new password
cat old.lock \
  | PND_PASSWORD=oldpass pnd-cli -m decrypt --stdout \
  | PND_PASSWORD=newpass pnd-cli -m encrypt --stdout \
  > new.lock

# Encrypt from stdin, write to a named file
cat report.pdf | PND_PASSWORD=s3cr3t pnd-cli -m encrypt -o report.pdf.lock
```

## Edge cases

| Situation | Behaviour |
|---|---|
| `<file>` does not exist | stderr error, exit 2 |
| `<file>` is a directory | stderr error, exit 3 |
| Output path already exists and `--force` not given | stderr error, exit 4 |
| Wrong password (decrypt auth fails) | Partial output deleted; exit 1 |
| Disk full mid-write | Partial output deleted; exit 2 |
| `-o` and `-t` combined | `-t` wins; `-o` is ignored with a warning |
| stdin piped, `--mode` not given | stderr error, exit 3 |
| `--stdout` and `--tui` combined | stderr error, exit 3 |
| Auth failure mid-stream to stdout | stderr error; exit 1; partial bytes already sent |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Wrong password / authentication failure |
| `2` | I/O or filesystem error |
| `3` | Bad arguments (e.g. `<file>` is a directory, missing `--mode`) |
| `4` | Output file exists and `--force` not given |

## Wire format

Each file is split into **64 MiB frames**. Every frame is independently encrypted with:

- A fresh 16-byte random salt
- A fresh 12-byte random IV
- AES-256-GCM with a key derived via PBKDF2-HMAC-SHA256 (100 000 iterations)

Each encrypted frame is preceded by a 4-byte big-endian size header. This layout lets
decryption stream one frame at a time without loading the entire file into memory.

Files encrypted by `pnd-cli` are fully compatible with the pnd-gui desktop application.
