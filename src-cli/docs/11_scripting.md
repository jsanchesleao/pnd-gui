# Scripting and Automation

`pnd-cli` is designed to work well in scripts and pipelines. This page collects patterns
and caveats for non-interactive use.

## Password via environment variable

All commands that require a password honour the `PND_PASSWORD` environment variable. When
set, `pnd-cli` uses its value as the password and skips the interactive prompt.

```bash
export PND_PASSWORD="my-vault-password"
pnd-cli --vault-list ~/vaults/work
```

A warning is always printed to **stderr** when `PND_PASSWORD` is used:

```
warning: using password from PND_PASSWORD environment variable
```

This ensures the variable's use is visible in logs. To suppress it, redirect stderr:

```bash
PND_PASSWORD=secret pnd-cli --vault-list 2>/dev/null
```

> **Security note:** environment variables are visible to other processes running as the
> same user on some operating systems. Prefer the interactive prompt when running on a
> shared machine. The variable is a scripting convenience, not a secure credential store.

## Exit codes

All non-interactive commands follow the same exit code contract:

```bash
pnd-cli report.pdf.lock
case $? in
  0) echo "Success" ;;
  1) echo "Wrong password" ;;
  2) echo "I/O error" ;;
  3) echo "Bad arguments" ;;
  4) echo "Output already exists" ;;
esac
```

## Suppressing progress output

Progress lines are written to stderr only when stderr is connected to a terminal. When
stderr is redirected or piped, no progress output is produced — the output stream stays
clean for processing.

```bash
# Encrypt quietly; only the result line goes to stdout
pnd-cli secret.txt 2>/dev/null
```

## JSON output for vault listing

Use `--json` with `--vault-list` to get machine-readable output:

```bash
# List all entry paths
pnd-cli --vault-list --json | jq -r '.[].path'

# Find entries larger than 10 MB
pnd-cli --vault-list --json | jq '[.[] | select(.size > 10485760)]'

# Count entries in a folder
pnd-cli --vault-list --path photos --json | jq 'length'
```

## Bulk export

```bash
# Export everything from the vault to a backup directory
PND_PASSWORD=secret pnd-cli --vault-export "" \
  --dest ~/backup/$(date +%Y-%m-%d) \
  --recursive --yes

# Export a specific folder
PND_PASSWORD=secret pnd-cli --vault-export documents \
  --dest ~/exports --recursive --yes --force
```

## Bulk add

```bash
# Add all JPEG files in a directory to a vault folder
PND_PASSWORD=secret pnd-cli --vault-add ~/photos/*.jpg \
  --vault-path photos/2024 --vault-dir ~/vaults/personal
```

## Non-interactive delete

The `-y` flag is required when stdin is not a TTY. Without it, the command exits 3 to
prevent accidental deletion in scripts that forget the flag:

```bash
# This will fail safely if -y is omitted and stdin is not a TTY
PND_PASSWORD=secret pnd-cli --vault-delete old/file.txt -y
```

## Checking for errors in pipelines

Use `set -e` or check `$?` explicitly after each `pnd-cli` invocation:

```bash
set -e
PND_PASSWORD=secret pnd-cli --vault-add report.pdf --vault-path documents
echo "Added successfully"
```

## Example: nightly vault backup script

```bash
#!/usr/bin/env bash
set -euo pipefail

VAULT_DIR=~/vaults/work
BACKUP_DIR=~/backups/vault-$(date +%Y-%m-%d)

mkdir -p "$BACKUP_DIR"

# Export the entire vault, skipping confirmation
PND_PASSWORD="${PND_PASSWORD:?PND_PASSWORD must be set}" \
  pnd-cli --vault-export "" \
    --dest "$BACKUP_DIR" \
    --recursive \
    --yes \
    --vault-dir "$VAULT_DIR" \
  2>&1 | tee -a ~/backups/backup.log

echo "Backup complete: $BACKUP_DIR"
```
