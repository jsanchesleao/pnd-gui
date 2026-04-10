# Interactive TUI

## Launching

```
pnd-cli
```

Running `pnd-cli` with no arguments opens the interactive terminal UI. It presents a
main menu with three pages:

- **Encrypt / Decrypt** — encrypt or decrypt a single file
- **Preview** — decrypt a file into memory and view it inline
- **Vault** — browse, manage, and search an encrypted vault

## Main menu navigation

| Key | Action |
|---|---|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `Enter` / `l` | Open the selected page |
| `q` / `Esc` | Quit |

## Encrypt / Decrypt page

Type or browse to a file path, then tab to the password field and press `Enter` to run
the operation. The mode (encrypt or decrypt) is detected automatically from the file
extension (`.lock` → decrypt, anything else → encrypt).

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Move between the path and password fields |
| `Enter` | Run the operation (from the password field) |
| `o` | Open the file browser to pick a file |
| `Esc` | Return to the main menu |

## Preview page

Same two-field layout as Encrypt / Decrypt. After the password is accepted the file is
decrypted into memory and the appropriate viewer is launched (Kitty inline image, mpv,
bat, or a built-in scrollable text viewer).

## Vault page

Opens the vault browser. See [07_vault_tui.md](07_vault_tui.md) for details.

## Pre-loading the TUI from the command line

The `-t`/`--tui` flag opens the TUI with a file path already filled in:

```bash
# Open Encrypt/Decrypt with the file path pre-populated
pnd-cli -t report.pdf

# Open Preview with the file path pre-populated
pnd-cli -p -t photo.jpg.lock
```

When the file is encrypted (`.lock`), focus lands on the password field so the user can
type their password immediately. When the file is plain (preview mode), decryption starts
automatically without a password prompt.
