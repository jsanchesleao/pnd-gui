# pnd-cli Documentation

## Contents

| File | Description |
|---|---|
| [01_overview.md](01_overview.md) | Tool overview, modes at a glance, exit codes, password input, wire format |
| [02_tui.md](02_tui.md) | Interactive TUI — main menu and page navigation |
| [03_encrypt_decrypt.md](03_encrypt_decrypt.md) | `pnd-cli <file>` — single-file encrypt / decrypt |
| [04_preview.md](04_preview.md) | `pnd-cli -p <file>` — preview a file (Kitty / mpv / bat) |
| [05_vault_concepts.md](05_vault_concepts.md) | Vault on-disk layout, cryptography, and virtual folder model |
| [06_vault_list.md](06_vault_list.md) | `--vault-list` — list vault contents (human-readable and JSON) |
| [07_vault_tui.md](07_vault_tui.md) | `--vault` — open vault in the TUI browser |
| [08_vault_preview_export.md](08_vault_preview_export.md) | `--vault-preview` and `--vault-export` |
| [09_vault_add.md](09_vault_add.md) | `--vault-add` — add files to a vault |
| [10_vault_rename_move_delete.md](10_vault_rename_move_delete.md) | `--vault-rename`, `--vault-move`, `--vault-delete` |
| [11_scripting.md](11_scripting.md) | Scripting patterns, `PND_PASSWORD`, JSON output, bulk operations |

## Quick reference

```
pnd-cli                                              # TUI main menu
pnd-cli <file>                                       # encrypt or decrypt
pnd-cli -m <enc|dec> [--stdout]                      # encrypt/decrypt from stdin
pnd-cli -p <file>                                    # preview
pnd-cli -p --ext <EXT> [-m decrypt]                  # preview from stdin
pnd-cli --vault [dir]                                # vault TUI browser
pnd-cli --vault-list [dir] [--json] [--path P]       # list vault contents
pnd-cli --vault-preview <vault-path> [dir]           # preview vault entry
pnd-cli --vault-export <vault-path> [--stdout]       # export vault entry
pnd-cli --vault-add <file>... [--vault-path P]       # add files to vault
pnd-cli --vault-add - --name <NAME>                  # add from stdin
pnd-cli --vault-rename <vault-path> <name>           # rename vault entry
pnd-cli --vault-move <vault-path> <folder>           # move vault entry
pnd-cli --vault-delete <vault-path>... [-y]          # delete vault entries
```
