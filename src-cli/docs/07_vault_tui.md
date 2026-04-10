# Vault — Interactive TUI Browser

## Synopsis

```
pnd-cli --vault [<vault-dir>]
```

## Description

Opens the vault at `<vault-dir>` directly in the TUI vault browser. The password is
prompted before the browser opens. This is equivalent to launching `pnd-cli`, selecting
Vault from the menu, and choosing Open — but in one step.

`<vault-dir>` defaults to the **current directory** when omitted.

## Options

| Flag | Description |
|---|---|
| `--vault-dir <DIR>` | Alternative to positional `<vault-dir>` |

## Examples

```bash
# Open the vault in the current directory
pnd-cli --vault

# Open a vault at a specific path
pnd-cli --vault ~/vaults/personal
```

## Vault browser layout

The vault browser is a two-panel layout:

- **Left panel** — virtual folder tree. Use arrow keys or `h`/`l` to navigate.
- **Right panel** — file list for the currently selected folder.

## Keyboard reference

### Navigation

| Key | Action |
|---|---|
| `Tab` | Switch focus between folder tree and file list |
| `↑` / `k` | Move up |
| `↓` / `j` | Move down |
| `→` / `l` / `Enter` | Expand folder / enter folder |
| `←` / `h` | Collapse folder / go to parent |
| `Esc` | Return to vault menu (or quit if launched with `--vault`) |

### File operations

| Key | Action |
|---|---|
| `a` | Add files (opens file picker) |
| `p` | Preview the selected file |
| `s` | Save (export) the selected file to disk |
| `r` | Rename the selected file |
| `x` | Cut the selected file(s) |
| `v` | Paste cut file(s) into the current folder |
| `Delete` / `d` | Delete the selected file(s) (shows confirmation dialog) |

### View and sort

| Key | Action |
|---|---|
| `g` | Toggle grid / list view |
| `n` | Sort by name |
| `t` | Sort by file type |
| `z` | Sort by size |
| `m` | Sort by date |
| `i` | Toggle ascending / descending order |

### Gallery mode

Pressing `G` (or the gallery button in the toolbar) opens a full-screen image/video
gallery for all entries in the current folder.

## Edge cases

| Situation | Behaviour |
|---|---|
| `<vault-dir>` does not exist | stderr error, exit 2 |
| `<vault-dir>` is not a vault (no `index.lock`) | stderr error, exit 2 |
| `<vault-dir>` is a file | stderr error, exit 3 |
| Wrong password | TUI error form shown; user can retry |

## Creating a new vault

From the vault menu (before unlocking), choose **New** to create a fresh vault in a
chosen directory. The vault directory must already exist; `pnd-cli` will create
`index.lock` inside it.
