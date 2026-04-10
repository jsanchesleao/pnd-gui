# Preview a File

## Synopsis

```
pnd-cli -p [OPTIONS] <file>
pnd-cli --preview [OPTIONS] <file>
```

## Description

Decrypts `<file>` entirely into memory — **never to disk** — and opens an appropriate
viewer for its content type. The file type is determined from the extension (after
stripping the `.lock` suffix for encrypted files).

If `<file>` does not have a `.lock` extension it is treated as a plain unencrypted file:
the content is read directly and no password is required.

### Viewer dispatch

| File type | Extensions | Viewer |
|---|---|---|
| Image | `jpg`, `jpeg`, `png`, `gif`, `webp`, `bmp`, `tiff` | Kitty inline graphics protocol; `xdg-open` fallback |
| Video | `mp4`, `mkv`, `avi`, `mov`, `webm`, `flv`, `wmv`, `m4v`, `ts`, `ogv` | mpv (blocks until closed) |
| Audio | `mp3`, `flac`, `wav`, `ogg`, `m4a`, `aac`, `opus`, `wma` | mpv (blocks until closed) |
| ZIP archive | `zip` | Inline image gallery (Kitty) or `xdg-open` |
| Text / code | `txt`, `md`, `json`, `yaml`, `toml`, `rs`, `py`, `js`, `ts`, `sh`, … | `bat` (syntax-highlighted); built-in scrollable viewer fallback |
| Other | anything else | "No previewer for `.<ext>` files" message; exit 0 |

#### Images (Kitty protocol)

On terminals that support the [Kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
(`$TERM=xterm-kitty`, `$TERM_PROGRAM=kitty`, or `$TERM_PROGRAM=wezterm`), images are
rendered inline scaled to fit the terminal's usable pixel area. Press any key to return
to the shell.

On other terminals, the image is opened with `xdg-open`. If `xdg-open` is not available,
an error is printed and the command exits with code 2.

#### Video and audio (mpv)

`pnd-cli` pipes the decrypted bytes to `mpv` via stdin, which takes over the terminal
for the duration of playback. When mpv exits, `pnd-cli` resumes. If `mpv` is not on
`$PATH`, an error is printed and the command exits with code 2.

#### ZIP image galleries

ZIP archives are opened as a keyboard-navigable image gallery on Kitty terminals. Each
image in the archive is decoded and displayed one at a time.

| Key | Action |
|---|---|
| `→` / `l` / `Space` | Next image |
| `←` / `h` | Previous image |
| `q` / `Esc` | Close gallery |

On non-Kitty terminals the ZIP is opened with `xdg-open`.

#### Text and code (`bat` / built-in viewer)

`bat` is invoked when available (syntax highlighting, line numbers, integrated pager).
When `bat` is not installed, a simple scrollable ratatui viewer is used instead.

Built-in viewer keys:

| Key | Action |
|---|---|
| `↑` / `k` | Scroll up |
| `↓` / `j` | Scroll down |
| `q` / `Esc` | Close |

## Options

| Flag | Description |
|---|---|
| `-t`, `--tui` | Open the TUI Preview screen with `<file>` pre-loaded instead of running non-interactively |

## Examples

```bash
# Preview an encrypted image
pnd-cli -p photo.jpg.lock

# Preview a plain markdown file (no password needed)
pnd-cli -p README.md

# Preview an encrypted video
pnd-cli -p movie.mp4.lock

# Open the TUI Preview screen with the file pre-loaded
pnd-cli -p --tui document.pdf.lock
```

## Edge cases

| Situation | Behaviour |
|---|---|
| `<file>` does not exist | stderr error, exit 2 |
| `<file>` is a directory | stderr error, exit 3 |
| Wrong password | stderr error, exit 1 |
| Unsupported file type | "No previewer for `.<ext>` files"; exit 0 |
| mpv not installed (media file) | Install hint on stderr; exit 2 |
| Non-Kitty terminal (image) | Falls back to `xdg-open`; if unavailable, exit 2 with hint |
| Ctrl-C during decrypt | Decrypted bytes never reach disk; exit 130 |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success (including "no previewer available" cases) |
| `1` | Wrong password / authentication failure |
| `2` | I/O error or missing external tool |
| `3` | Bad arguments |
