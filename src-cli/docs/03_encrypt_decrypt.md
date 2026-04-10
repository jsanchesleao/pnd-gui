# Encrypt / Decrypt a Single File

## Synopsis

```
pnd-cli [OPTIONS] <file>
```

## Description

Encrypts or decrypts a single file. The mode is detected automatically from the file
extension:

- **`.lock` extension** → decrypt. Output is `<file>` with the `.lock` suffix removed.
- **Any other extension** → encrypt. Output is `<file>.lock`.

The password is read from the terminal (hidden) or from the `PND_PASSWORD` environment
variable. See [01_overview.md](01_overview.md) for password input details.

Output is written atomically: a temporary file is created in the same directory as the
output path, then renamed into place on success. If the operation fails at any point the
temporary file is deleted and the destination path is left untouched.

Progress is reported on stderr when stderr is connected to a terminal. The line is
overwritten in place (`\r`) so it does not scroll. When stderr is redirected or piped,
progress output is suppressed entirely.

## Options

| Flag | Description |
|---|---|
| `-o <PATH>` | Write output to `PATH` instead of the default location |
| `-f`, `--force` | Overwrite the output file if it already exists |
| `-t`, `--tui` | Open the TUI Encrypt/Decrypt screen with `<file>` pre-loaded |

## Examples

```bash
# Encrypt a file (produces report.pdf.lock)
pnd-cli report.pdf

# Decrypt a file (produces report.pdf)
pnd-cli report.pdf.lock

# Encrypt and write to a custom path
pnd-cli report.pdf -o /backups/report.enc

# Encrypt, overwriting an existing output file
pnd-cli report.pdf --force

# Use environment variable for scripting
PND_PASSWORD=s3cr3t pnd-cli report.pdf
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

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Wrong password / authentication failure |
| `2` | I/O or filesystem error |
| `3` | Bad arguments (e.g. `<file>` is a directory) |
| `4` | Output file exists and `--force` not given |

## Wire format

Each file is split into **64 MiB frames**. Every frame is independently encrypted with:

- A fresh 16-byte random salt
- A fresh 12-byte random IV
- AES-256-GCM with a key derived via PBKDF2-HMAC-SHA256 (100 000 iterations)

Each encrypted frame is preceded by a 4-byte big-endian size header. This layout lets
decryption stream one frame at a time without loading the entire file into memory.

Files encrypted by `pnd-cli` are fully compatible with the pnd-gui desktop application.
