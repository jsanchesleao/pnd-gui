# pnd-cli — Overview

`pnd-cli` is the command-line companion to **pnd-gui** (Password & Note Depot). It encrypts,
decrypts, previews, and manages encrypted vaults — all using the same wire format as the
desktop GUI application, so files created by one tool are fully interoperable with the other.

## Modes at a glance

| Invocation | Mode |
|---|---|
| `pnd-cli` | Interactive TUI (main menu) |
| `pnd-cli <file>` | Non-interactive encrypt or decrypt |
| `pnd-cli -m <enc\|dec> [--stdout]` | Encrypt or decrypt from stdin |
| `pnd-cli -p <file>` | Non-interactive file preview |
| `pnd-cli -p --ext <EXT> [-m decrypt]` | Preview from stdin |
| `pnd-cli --vault [dir]` | Open vault in the TUI |
| `pnd-cli --vault-list [dir]` | List vault contents |
| `pnd-cli --vault-preview <path>` | Preview a vault entry |
| `pnd-cli --vault-export <path> [--stdout]` | Export a vault entry to disk or stdout |
| `pnd-cli --vault-add <file>...` | Add files to a vault |
| `pnd-cli --vault-add - --name <name>` | Add stdin content to a vault |
| `pnd-cli --vault-rename <path> <name>` | Rename a vault entry |
| `pnd-cli --vault-move <path> <folder>` | Move a vault entry |
| `pnd-cli --vault-delete <path>...` | Delete vault entries |

## TUI vs non-interactive

Any command that does not open the TUI is called **non-interactive**: it reads a password,
does its work, prints a result line to stdout, and exits.  The `-t`/`--tui` flag can
override this for the encrypt/decrypt and preview commands, launching the TUI instead.

## Global options

These flags are accepted by every command:

| Flag | Description |
|---|---|
| `-h`, `--help` | Print usage information and exit |
| `--version` | Print the version string and exit |
| `--vault-dir <DIR>` | Alternative to a positional `<vault-dir>` for all vault commands |

## Exit codes

All non-interactive modes follow a consistent set of exit codes:

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Wrong password or decryption authentication failure |
| `2` | I/O or filesystem error |
| `3` | Bad arguments or usage error |
| `4` | Output file already exists (only when `--force` is not given) |

## Password input

Passwords are **never accepted as a plain CLI argument** (they would be visible in `ps`
output and shell history).

Instead, `pnd-cli` reads the password one of two ways (in priority order):

1. **`PND_PASSWORD` environment variable** — useful for scripting. A warning is always
   printed to stderr when this variable is used:
   ```
   warning: using password from PND_PASSWORD environment variable
   ```
2. **Interactive hidden prompt** — the default. `pnd-cli` prints `Password:` to the
   terminal and reads the input with echo disabled. This requires a real TTY; the prompt
   will fail if stdin is redirected.

## Atomic writes

Every operation that writes to disk does so **atomically**: the output is first written to
a temporary file in the same directory, then renamed into place. This means a crash or
disk-full condition never leaves a partial or corrupted output file at the destination
path.

## Wire format compatibility

The CLI uses the same encryption format as the GUI:

- **Single-file format** (encrypt/decrypt/preview): PBKDF2-HMAC-SHA256 (100 000
  iterations) for key derivation, AES-256-GCM per 64 MiB frame. Each frame carries its
  own random salt and IV, prefixed by a 4-byte big-endian size header.
- **Vault format**: the `index.lock` index file uses the same password-derived key scheme;
  blob files use a per-blob random AES-256 key stored (base64-encoded) in the index entry.

See the individual command pages for detailed usage.
