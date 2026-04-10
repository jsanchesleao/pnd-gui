# Piping Input and Output

This document specifies how `pnd-cli` interacts with Unix pipes — reading plaintext or
ciphertext from stdin, and writing plaintext or ciphertext to stdout. It covers the full
behaviour contract, conflicts with existing code, edge cases, and an implementation
roadmap.

---

## Overview

Piping is divided into two independent axes:

| Direction | What it means | Trigger |
|---|---|---|
| **Stdin piping** | Another process feeds bytes into `pnd-cli` | stdin is not a TTY |
| **Stdout piping** | `pnd-cli` writes its output bytes to the next process | `--stdout` flag |

These axes are orthogonal: you can pipe in, pipe out, both, or neither.

---

## Stdin piping

### Supported operations

| Operation | Command | Notes |
|---|---|---|
| Encrypt | `cat file.txt \| pnd-cli -m encrypt` | `--mode` required |
| Decrypt | `cat file.lock \| pnd-cli -m decrypt` | `--mode` required |
| Preview | `cat file.jpg.lock \| pnd-cli -p --ext jpg` | `--ext` required |
| Vault add | `cat file.pdf \| pnd-cli --vault-add - --name report.pdf` | `--name` required |

### Why `--mode` / `-m` is required for encrypt/decrypt

Currently, mode is auto-detected from the input file extension (`.lock` → decrypt,
anything else → encrypt). When input comes from stdin there is no filename, so the
extension cannot be inspected. The `--mode` / `-m` flag makes the intent explicit.

```
pnd-cli -m encrypt           # encrypt stdin → stdout (with --stdout) or file (-o)
pnd-cli -m decrypt           # decrypt stdin → stdout or file (-o)
```

**Valid values:** `encrypt` (alias `enc`, `e`) and `decrypt` (alias `dec`, `d`).

If stdin is not a TTY and `--mode` is not given, print an error to stderr and exit 3.

### Why `--ext` is required for preview

The preview pipeline dispatches on file extension (image → Kitty, video → mpv, text →
bat). With no filename there is no extension. The `--ext` flag provides it:

```
cat photo.jpg.lock | pnd-cli -p --ext jpg
cat song.mp3.lock  | pnd-cli -p --ext mp3
cat notes.txt      | pnd-cli -p --ext txt   # plain file, no password needed
```

The value must be a bare extension without a leading dot. If stdin is not a TTY and
`--ext` is not given, print an error to stderr and exit 3.

A plain (non-encrypted) stdin stream bypasses the password prompt when combined with
`-p`. Whether a stream is encrypted is inferred: if `--mode decrypt` is given (or the
source is `.lock`), it is encrypted; otherwise it is plain.

### Why `--name` is required for `--vault-add`

The vault's `index.lock` stores a `name` field for every entry. When adding a file from
disk, the name is taken from the filename. When the source is stdin (`-`), there is no
filename, so `--name` must be supplied:

```
cat report.pdf | pnd-cli --vault-add - --name report.pdf --vault-path documents
```

If stdin is not a TTY and the source is `-` but `--name` is absent, print an error and
exit 3.

### Password prompt and stdin conflicts

`rpassword::prompt_password` (used by `read_password()`) opens `/dev/tty` directly on
Unix instead of reading from stdin. This means the interactive password prompt **does
work** even when stdin is a pipe — the prompt appears on the terminal and the piped data
is unaffected.

However, `/dev/tty` may not exist in all environments (containers, CI, headless systems).
In those cases `rpassword` will return an error. `read_password()` already exits with
code 2 on that failure path, which remains the correct behaviour.

`PND_PASSWORD` remains the recommended approach for fully non-interactive pipelines where
a TTY cannot be guaranteed:

```bash
cat secret.txt | PND_PASSWORD=mypass pnd-cli -m encrypt --stdout > secret.lock
```

### Progress output when reading from stdin

Currently `enc_dec_cli.rs` reads `input_path.metadata().len()` to compute progress
percentage. stdin has no known size, so percentage progress cannot be calculated.

When reading from stdin:
- If stderr is a TTY: print byte count only — `Encrypting… 4.2 MB\r`
- If stderr is not a TTY: suppress progress entirely (no output to stderr)

This avoids displaying a meaningless `0%` or dividing by zero.

---

## Stdout piping

### Supported operations

| Operation | Command | Notes |
|---|---|---|
| Encrypt | `pnd-cli -m encrypt file.txt --stdout` | writes `.lock` bytes to stdout |
| Decrypt | `pnd-cli file.txt.lock --stdout` | writes plaintext bytes to stdout |
| Vault export | `pnd-cli --vault-export notes.txt --stdout` | single entry only |

### The `--stdout` / `-c` flag

Add a `--stdout` (short: `-c`) flag. When set:
- Output is written directly to `io::stdout()` instead of a file.
- The atomic temp-file write is **not** used (there is nothing to rename).
- The `-o` flag is incompatible with `--stdout`; if both are given, print a warning and
  let `--stdout` win.
- Progress output is suppressed (stdout carries data, not status).

The flag name `-c` mirrors tools like `gzip -c` and `zcat` (write to stdout).

### Vault export stdout restriction

`--vault-export` with `--stdout` is only permitted for a **single entry** (a file path,
not a folder path). If the resolved vault path is a folder — or if `-r`/`--recursive`
is given alongside `--stdout` — print an error and exit 3. This prevents ambiguous
multi-file byte streams that cannot be demultiplexed by the receiving process.

### Partial output on auth failure

When writing to a file, the atomic temp-rename ensures the destination is never
partially written. When writing to stdout, the stream is live and cannot be rolled back.
If authentication fails mid-stream (during a multi-frame decrypt):

1. An error message is printed to **stderr**.
2. `pnd-cli` exits with code 1.
3. The pipe is broken; the receiving process sees EOF or `SIGPIPE`.

The caller is responsible for not using partial output. This is consistent with how
`openssl enc`, `gpg`, and similar tools behave.

### Progress and stdout

When stdout carries data (piped output), progress lines must **not** be written to
stdout. They may still be written to stderr when stderr is a TTY. In practice, if stdout
is piped it is very common for stderr also to be piped or redirected; the existing
`is_terminal()` check on stderr handles this correctly already.

---

## Combining stdin and stdout

The full streaming pipeline:

```bash
# Encrypt from stdin to stdout
cat report.pdf | PND_PASSWORD=s3cr3t pnd-cli -m encrypt --stdout > report.pdf.lock

# Decrypt from stdin to stdout
cat report.pdf.lock | PND_PASSWORD=s3cr3t pnd-cli -m decrypt --stdout

# Chain: decrypt then re-encrypt with a new password
cat old.lock \
  | PND_PASSWORD=oldpass pnd-cli -m decrypt --stdout \
  | PND_PASSWORD=newpass pnd-cli -m encrypt --stdout \
  > new.lock
```

When both stdin and stdout are piped:
- Progress output is suppressed (stderr is likely redirected too, but the `is_terminal()`
  guard handles that).
- Password must come from `PND_PASSWORD` or a reachable `/dev/tty`.

---

## New CLI flags summary

| Flag | Short | Type | Used with | Description |
|---|---|---|---|---|
| `--mode` | `-m` | `encrypt` \| `decrypt` | stdin enc/dec | Required when stdin is not a TTY and no file is given |
| `--ext` | — | `<EXT>` | stdin preview | Required when piping into `-p` (bare extension, no dot) |
| `--stdout` | `-c` | bool | enc/dec, vault export | Write output to stdout instead of a file |

`--name` already exists in `cli.rs` (added for `--vault-move`). It is **reused** for
`--vault-add -` to supply the entry name when the source is stdin. The logic is: if
`--vault-add` receives `-` as the sole file argument, treat stdin as the source and
require `--name`.

---

## Code conflicts and required changes

### 1. `src/cli.rs` — new flags

Add `--mode`, `--ext`, and `--stdout` to the `Cli` struct:

```rust
/// Explicit operation mode (required when reading from stdin)
#[arg(short = 'm', long, value_enum)]
pub mode: Option<OperationMode>,

/// File extension for stdin preview dispatch (e.g. "jpg", "mp3")
#[arg(long, value_name = "EXT")]
pub ext: Option<String>,

/// Write output to stdout instead of a file
#[arg(short = 'c', long)]
pub stdout: bool,
```

Add the `OperationMode` enum:

```rust
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum OperationMode {
    #[value(alias = "enc", alias = "e")]
    Encrypt,
    #[value(alias = "dec", alias = "d")]
    Decrypt,
}
```

Update `is_tui_mode()` to return `false` when `cli.stdout` is set or stdin is not a
TTY with `--mode` set (those are always non-interactive).

**Conflict:** `--name` already exists and is documented as "used with `--vault-move`".
The flag's description should be updated to cover both uses:
`"Name for the vault entry (used with --vault-move to rename while moving, and with
--vault-add - when reading from stdin)"`.

**Conflict:** `-o` and `--stdout` are mutually exclusive. Clap `conflicts_with` or a
manual check in `main.rs` should enforce this. Use a warning + `--stdout` wins (matches
the existing `-o` + `--tui` precedence pattern).

### 2. `src/enc_dec_cli.rs` — stdin source and stdout sink

**Current behaviour:** opens `cli.files[0]` and writes to a temp file that is renamed.

**Required changes:**

- Detect stdin source: `cli.files.is_empty() || cli.files[0] == Path::new("-")`.
  - When stdin source: require `cli.mode` to be set; derive is-decrypt from it.
  - When stdin source: skip the `metadata().len()` call; use byte-count-only progress.
  - Open `io::stdin()` as the reader instead of `fs::File::open`.

- Detect stdout sink: `cli.stdout`.
  - When stdout sink: write directly to `io::stdout()`; skip temp-file/rename path.
  - When stdout sink: suppress progress output unconditionally.
  - When stdout sink: on failure, print error to stderr and exit — partial bytes have
    already been sent; no cleanup is possible.

- Output path logic when `--stdout` is set: skip all output-path collision checks and
  the `-o` flag entirely (with a warning if `-o` was also given).

### 3. `src/preview_cli.rs` — stdin source and `--ext`

**Current behaviour:** opens `cli.files[0]`, derives extension from the filename after
stripping `.lock`.

**Required changes:**

- Detect stdin source: same `-` or empty files check as enc_dec_cli.
- When stdin source:
  - Require `cli.ext` to be set; use it as the dispatch extension.
  - Read all of stdin into `Vec<u8>`.
  - If `cli.mode == Some(Decrypt)` (or source appears to be `.lock`), run
    `decrypt_file` on the buffer; otherwise treat as plain bytes.
  - Pass `bytes` and `ext` into `PreviewPhase::PendingRender` as usual — no other
    change to the preview pipeline is needed.

### 4. `src/vault_add_cli.rs` — stdin source

**Current behaviour:** iterates `cli.vault_add` as file paths.

**Required changes:**

- Detect stdin source: if the single element of `cli.vault_add` is `Path::new("-")`.
- When stdin source:
  - Require `cli.name` to be set; use it as the vault entry's `name` field.
  - Read all of stdin into `Vec<u8>`.
  - Compute `size` from the buffer length.
  - Call the existing `encrypt_file_to_vault` path after writing the bytes to a temp
    file, OR add a new `encrypt_bytes_to_vault(bytes, blobs_dir, vault_path)` helper
    that accepts an already-buffered `&[u8]`. The latter is cleaner and avoids a temp
    file.
  - Multiple `-` sources in one invocation is an error (exit 3).

**Conflict:** `VaultEntry.size` is currently set by `encrypt_file_to_vault` which reads
the filesystem metadata. When the source is stdin, size must be computed from the in-memory
buffer length after reading all of stdin. The `encrypt_bytes_to_vault` helper should
accept a `u64 size` parameter or derive it from the slice length.

### 5. `src/vault_op_cli.rs` — stdout sink for `--vault-export`

**Current behaviour:** always writes to `dest_dir / entry.name`.

**Required changes:**

- When `cli.stdout` is set:
  - Refuse if the vault path resolves to a folder (not a single entry). Exit 3.
  - Refuse if `-r`/`--recursive` is also set. Exit 3.
  - Decrypt the entry's blobs into memory as usual.
  - Write the plaintext bytes directly to `io::stdout()` instead of calling
    `write_atomic`.
  - Skip all destination-collision checks.
  - Skip the confirmation prompt.

### 6. `src/password.rs` — no change required

`rpassword::prompt_password` already opens `/dev/tty` directly on Unix. No change is
needed for the common case. The existing error path (exits 2 when the prompt fails)
covers the headless case.

Document this explicitly in `read_password()` with a comment explaining that the function
does not consume stdin.

---

## Edge cases

| Situation | Expected behaviour |
|---|---|
| `--mode` not given when stdin is not a TTY | stderr error: "stdin is not a TTY; use -m to specify encrypt or decrypt", exit 3 |
| `-p` without `--ext` when stdin is not a TTY | stderr error: "--ext is required when piping into -p", exit 3 |
| `--vault-add -` without `--name` | stderr error: "--name is required when adding from stdin", exit 3 |
| `--vault-add -` with multiple files | stderr error: "cannot combine stdin source (-) with other files", exit 3 |
| `--stdout` with `--tui` | stderr error: "--stdout is incompatible with --tui", exit 3 |
| `--stdout` with `-o` | `-o` ignored, warning printed to stderr; `--stdout` wins |
| `--stdout` with `--vault-export` folder path | stderr error: "--stdout requires a single-file vault path", exit 3 |
| `--stdout` with `-r` | stderr error: "--stdout is incompatible with --recursive", exit 3 |
| Auth failure mid-stream to stdout | stderr error; exit 1; pipe broken (partial bytes already sent) |
| stdin is a TTY and no file given | Existing behaviour: launch TUI (no change) |
| stdin is a pipe but `--tui` is set | stderr error: "--tui cannot be used when stdin is a pipe", exit 3 |
| stdin EOF before first byte (empty pipe) | Encrypt: write an empty-file ciphertext (one zero-length frame, matching existing behaviour). Decrypt: exit 0 with no output. |
| Very large stdin (multi-frame) | Handled naturally by the existing 64 MiB frame loop — no full buffer required |
| `/dev/tty` unavailable (container/CI) with no `PND_PASSWORD` | `rpassword` fails; `read_password()` exits 2 with error message |
| SIGPIPE from downstream process | Rust's default SIGPIPE handling causes the `write_all` to return an `io::Error`; the error path exits 2 cleanly |
| `--vault-add -` size field | `VaultEntry.size` is set to `stdin_bytes.len() as u64` after buffering |
| Progress divides by total size = 0 (stdin encrypt) | Show byte-count-only progress instead of percentage; existing `if total == 0 { return; }` guard in `report_progress` already prevents division by zero |
| Interleaved stderr progress and stdout data | Progress goes to stderr, data to stdout; they are independent file descriptors — no interleaving issue |

---

## Verification scenarios

Each scenario should be exercised manually and, where possible, covered by an integration
test using `std::process::Command` with `stdin(Stdio::piped())` / `stdout(Stdio::piped())`.

### Encrypt / decrypt round-trip via pipe

```bash
# 1. Encrypt file to stdout, decrypt back, compare
echo "hello world" \
  | PND_PASSWORD=test pnd-cli -m encrypt --stdout \
  | PND_PASSWORD=test pnd-cli -m decrypt --stdout
# Expected: prints "hello world\n"

# 2. Binary data round-trip
dd if=/dev/urandom bs=1M count=5 2>/dev/null \
  | PND_PASSWORD=test pnd-cli -m encrypt --stdout \
  | PND_PASSWORD=test pnd-cli -m decrypt --stdout \
  | sha256sum
# Expected: hash matches the original random data

# 3. Wrong password during decrypt
echo "data" | PND_PASSWORD=correct pnd-cli -m encrypt --stdout \
  | PND_PASSWORD=wrong pnd-cli -m decrypt --stdout
# Expected: exit 1, error on stderr, no output on stdout
```

### Encrypt from stdin to file

```bash
# 4. Output to named file
echo "test" | PND_PASSWORD=pw pnd-cli -m encrypt -o out.lock
pnd-cli out.lock   # decrypt interactively, enter "pw"
# Expected: file out.lock created; decryption yields "test\n"
```

### Decrypt from file to stdout

```bash
# 5. Decrypt to stdout and pipe to another tool
pnd-cli secret.txt.lock --stdout | grep "keyword"
# Expected: matched lines printed; exit code from grep
```

### Preview from stdin

```bash
# 6. Pipe an encrypted image for preview
cat photo.jpg.lock | PND_PASSWORD=pw pnd-cli -p --ext jpg
# Expected: Kitty image displayed (or xdg-open fallback)

# 7. Pipe a plain text file
cat README.md | pnd-cli -p --ext md
# Expected: bat/viewer opens with the markdown content
```

### Vault add from stdin

```bash
# 8. Add generated content to vault
date | PND_PASSWORD=pw pnd-cli --vault-add - --name timestamp.txt
pnd-cli --vault-list   # enter pw
# Expected: "timestamp.txt" appears in the listing

# 9. Missing --name exits with code 3
echo "data" | PND_PASSWORD=pw pnd-cli --vault-add -
# Expected: exit 3, error message on stderr
```

### Vault export to stdout

```bash
# 10. Export single entry to stdout
PND_PASSWORD=pw pnd-cli --vault-export notes.txt --stdout | wc -c
# Expected: prints plaintext byte count

# 11. Refuse to pipe a folder
PND_PASSWORD=pw pnd-cli --vault-export photos --stdout
# Expected: exit 3, error on stderr

# 12. Combine: export from vault, re-encrypt as a single file
PND_PASSWORD=vaultpw pnd-cli --vault-export report.pdf --stdout \
  | PND_PASSWORD=filepw pnd-cli -m encrypt --stdout > report.pdf.lock
# Expected: report.pdf.lock decryptable with "filepw"
```

### Incompatible flag combinations

```bash
# 13. --stdout + --tui
pnd-cli --stdout --tui file.txt
# Expected: exit 3

# 14. --stdout + --recursive
PND_PASSWORD=pw pnd-cli --vault-export photos --stdout -r
# Expected: exit 3

# 15. Missing --mode when stdin is a pipe
echo "data" | pnd-cli
# Expected: exit 3 (not TUI launch)

# 16. Missing --ext when piping to -p
cat photo.jpg.lock | PND_PASSWORD=pw pnd-cli -p
# Expected: exit 3
```

---

## Implementation roadmap

### Phase 10-A — Stdout output (`--stdout` / `-c`)

Scope: output side only, no stdin changes. Lowest risk because it does not touch the
password or input path.

1. Add `--stdout` / `-c` flag to `Cli` in `cli.rs`.
2. In `enc_dec_cli.rs`: when `cli.stdout` is set, write to `io::stdout()` instead of
   temp file; suppress progress; handle write errors directly (no temp-file cleanup).
3. In `vault_op_cli.rs` (`run_export`): when `cli.stdout` is set, refuse folder exports
   and `-r`; decrypt single entry; write to stdout.
4. Update `is_tui_mode()` to return `false` when `cli.stdout` is set.
5. Tests: integration tests for encrypt-to-stdout and vault-export-to-stdout.

**Acceptance:** `pnd-cli file.txt.lock --stdout | wc -c` prints the plaintext byte count.

---

### Phase 10-B — Stdin for encrypt/decrypt (`--mode` / `-m`)

Scope: reading from stdin for the encrypt/decrypt command. Depends on 10-A for the
`--stdout` output path.

1. Add `--mode` / `-m` (`OperationMode` enum) to `Cli`.
2. In `enc_dec_cli.rs`:
   - Detect stdin source (`files.is_empty()` and stdin not a TTY, or `files[0] == "-"`).
   - When stdin: require `--mode`; use `io::stdin()` as reader; use byte-count progress.
   - Reject `--tui` when stdin is a pipe.
3. Tests: roundtrip via `Command` with `stdin(Stdio::piped())`.

**Acceptance:** `echo "hello" | PND_PASSWORD=pw pnd-cli -m encrypt --stdout | PND_PASSWORD=pw pnd-cli -m decrypt --stdout` prints `hello`.

---

### Phase 10-C — Stdin for preview (`--ext`)

Scope: reading from stdin for `-p`. Depends on 10-B for the stdin-detection pattern.

1. Add `--ext` to `Cli`.
2. In `preview_cli.rs`:
   - Detect stdin source.
   - When stdin: require `--ext`; read all bytes; dispatch on extension as usual.
   - Determine whether to decrypt based on `--mode decrypt` presence.
3. Tests: pipe a small test image through `pnd-cli -p --ext png`.

**Acceptance:** `cat photo.png | pnd-cli -p --ext png` renders the image (or exits 0
with "No previewer" if running in a non-Kitty CI environment).

---

### Phase 10-D — Stdin for `--vault-add`

Scope: reading from stdin for `--vault-add`. Most complex because it touches vault
index writing.

1. Detect `vault_add == ["-"]` in `main.rs`; route to updated `vault_add_cli::run_add`.
2. In `vault_add_cli.rs`:
   - When source is `-`: require `cli.name`; read stdin to `Vec<u8>`.
   - Add `encrypt_bytes_to_vault(bytes: &[u8], name: &str, blobs_dir, vault_path)`
     helper in `vault/crypto.rs` (or inline in `vault_add_cli.rs`). This replaces the
     `encrypt_file_to_vault` call for the stdin case.
   - Set `VaultEntry.size` from `bytes.len() as u64`.
3. Tests: pipe known content, re-export, verify content matches.

**Acceptance:** `echo "secret" | PND_PASSWORD=pw pnd-cli --vault-add - --name secret.txt && PND_PASSWORD=pw pnd-cli --vault-export secret.txt --stdout` prints `secret`.
