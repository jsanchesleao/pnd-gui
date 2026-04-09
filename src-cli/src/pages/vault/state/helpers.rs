//! Pure helper functions for the vault state machine.

use std::cmp::Ordering;
use super::{SortDir, SortKey};
use crate::pages::vault::types::VaultHandle;

/// Compute the sort ordering for two file entries given their (name, size,
/// file-category ordinal) and the current sort key + direction.
/// `SortKey::Age` is handled at the call site (reversal only) — do not pass it here.
pub(super) fn sort_order(
    na: &str, sa: u64, ca: u8,
    nb: &str, sb: u64, cb: u8,
    key: SortKey, dir: SortDir,
) -> Ordering {
    let ord = match key {
        SortKey::Name => na.cmp(nb),
        SortKey::Size => sa.cmp(&sb),
        SortKey::Type => ca.cmp(&cb).then_with(|| na.cmp(nb)),
        SortKey::Age  => Ordering::Equal, // handled at call site
    };
    if dir == SortDir::Desc { ord.reverse() } else { ord }
}

/// For each UUID in `uuids`, find a name that does not conflict with any
/// existing entry at `dest` in `handle`. Returns `(uuid, final_name)` pairs
/// in the same order as `uuids`.
pub(super) fn resolve_names(
    uuids: &[String],
    dest: &str,
    handle: &VaultHandle,
) -> Vec<(String, String)> {
    let mut resolved: Vec<(String, String)> = Vec::new();
    for uuid in uuids {
        let base = handle.index.entries.get(uuid)
            .map(|e| e.name.clone())
            .unwrap_or_default();
        let mut final_name = base.clone();
        let mut counter = 1u32;
        loop {
            let conflict = handle.index.entries.iter()
                .filter(|(u, _)| *u != uuid)
                .any(|(_, e)| e.path == dest && e.name == final_name);
            if !conflict { break; }
            let (stem, ext) = split_name(&base);
            final_name = if ext.is_empty() {
                format!("{stem} ({counter})")
            } else {
                format!("{stem} ({counter}).{ext}")
            };
            counter += 1;
        }
        resolved.push((uuid.clone(), final_name));
    }
    resolved
}

/// Collect all unique folder paths implied by the vault index entries, plus
/// the root path `""`.
pub(super) fn collect_all_folders(handle: &VaultHandle) -> Vec<String> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    seen.insert(String::new()); // root is always present
    for entry in handle.index.entries.values() {
        let mut path = entry.path.clone();
        loop {
            if !seen.insert(path.clone()) { break; } // already present — parents too
            match path.rfind('/') {
                Some(pos) => { path = path[..pos].to_string(); }
                None => { seen.insert(String::new()); break; }
            }
        }
    }
    seen.into_iter().collect()
}

/// Split a filename into `(stem, extension)`. Returns `(name, "")` when there
/// is no extension or the dot is at position 0 (hidden files like `.gitignore`).
pub(super) fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(pos) if pos > 0 => (&name[..pos], &name[pos + 1..]),
        _ => (name, ""),
    }
}
