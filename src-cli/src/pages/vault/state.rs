//! Vault page state machine, background unlock worker, and in-memory operations.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use super::types::{VaultEntry, VaultHandle};

// ── Worker messages ────────────────────────────────────────────────────────

pub(super) enum WorkerMsg {
    Progress(u8),
    Opened(VaultHandle),
    Created(VaultHandle),
    Failed(String),
}

pub(super) enum AddWorkerMsg {
    /// `done` files encrypted so far out of `total`, currently working on `filename`.
    Progress { done: usize, total: usize, filename: String },
    /// All files encrypted; payload is the new (uuid, entry) pairs to merge.
    Done(Vec<(String, VaultEntry)>),
    /// Unrecoverable error during add.
    Failed(String),
}

pub(super) enum PreviewWorkerMsg {
    /// Decrypted bytes ready for rendering, with the file's lowercased extension.
    Ready(Vec<u8>, String),
    Failed(String),
}

pub(super) enum ExportWorkerMsg {
    Progress { done: usize, total: usize, filename: String },
    /// Number of files successfully exported.
    Done(usize),
    Failed(String),
}

pub(super) enum GalleryWorkerMsg {
    Progress { done: usize, total: usize },
    /// All images decrypted; payload is `(entry name, raw bytes)` sorted by name.
    Ready(Vec<(String, Vec<u8>)>),
    Failed(String),
}

// ── Sort ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortKey {
    Name,
    Size,
    Type,
    /// Insertion order in the vault index — chronological (oldest first for Asc).
    Age,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortDir {
    Asc,
    Desc,
}

impl SortKey {
    pub(crate) fn label(self) -> &'static str {
        match self {
            SortKey::Name => "Name",
            SortKey::Size => "Size",
            SortKey::Type => "Type",
            SortKey::Age  => "Age",
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            SortKey::Name => SortKey::Size,
            SortKey::Size => SortKey::Type,
            SortKey::Type => SortKey::Age,
            SortKey::Age  => SortKey::Name,
        }
    }
}

impl SortDir {
    pub(crate) fn arrow(self) -> &'static str {
        match self { SortDir::Asc => "↑", SortDir::Desc => "↓" }
    }

    pub(crate) fn toggle(self) -> Self {
        match self { SortDir::Asc => SortDir::Desc, SortDir::Desc => SortDir::Asc }
    }
}

/// Map a filename to a category ordinal for type-based sorting (lower = shown first).
pub(crate) fn file_category(name: &str) -> u8 {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "tif"
        | "svg" | "ico" | "avif" | "heic" => 0,
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "m4v" => 1,
        "mp3" | "flac" | "wav" | "ogg" | "aac" | "m4a" | "opus" | "wma" => 2,
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "xls" | "xlsx" | "ods"
        | "ppt" | "pptx" | "odp" => 3,
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "lz4" => 4,
        "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "html" | "css" | "json"
        | "toml" | "yaml" | "yml" | "xml" | "sh" | "bash" | "zsh" | "fish"
        | "md" | "txt" | "c" | "cpp" | "h" | "hpp" | "go" | "java" | "rb"
        | "php" | "lua" | "vim" | "swift" | "kt" | "cs" | "sql" => 5,
        _ => 6,
    }
}

// ── Panels ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelFocus {
    Tree,
    List,
}

// ── Browse state ───────────────────────────────────────────────────────────

pub(crate) struct BrowseState {
    pub(crate) handle: VaultHandle,
    /// Virtual folder path the user is currently viewing (empty = root).
    pub(crate) current_path: String,
    /// Immediate sub-folder names under `current_path`, sorted.
    pub(crate) folders: Vec<String>,
    /// UUIDs of files in `current_path`, sorted by entry name.
    pub(crate) file_uuids: Vec<String>,
    /// UUIDs toggled with Space.
    pub(crate) selected_uuids: HashSet<String>,
    /// Cursor in the right-panel combined list (folders first, then files).
    pub(crate) list_cursor: usize,
    /// Flat sorted list of every folder path in the vault (including root "").
    pub(crate) all_folders: Vec<String>,
    /// Cursor in the left-panel folder tree.
    pub(crate) tree_cursor: usize,
    pub(crate) panel_focus: PanelFocus,
    /// UUIDs staged for a paste (move) operation.
    pub(crate) clipboard: Vec<String>,
    /// Index has unsaved changes.
    pub(crate) dirty: bool,
    /// Transient one-line feedback shown in the hint bar.
    pub(crate) status_msg: Option<String>,
    /// When `status_msg` was set (for auto-clearing after a timeout).
    pub(crate) status_msg_at: Option<Instant>,
    /// Folders that were created in this session but are still empty (not yet
    /// derivable from the index). Merged into `all_folders` during `refresh`.
    pub(crate) extra_folders: Vec<String>,
    /// Current sort key for the file list.
    pub(crate) sort_key: SortKey,
    /// Current sort direction for the file list.
    pub(crate) sort_dir: SortDir,
}

impl BrowseState {
    pub(crate) fn new(handle: VaultHandle) -> Self {
        let mut s = BrowseState {
            handle,
            current_path: String::new(),
            folders: Vec::new(),
            file_uuids: Vec::new(),
            selected_uuids: HashSet::new(),
            list_cursor: 0,
            all_folders: Vec::new(),
            tree_cursor: 0,
            panel_focus: PanelFocus::List,
            clipboard: Vec::new(),
            dirty: false,
            status_msg: None,
            status_msg_at: None,
            extra_folders: Vec::new(),
            sort_key: SortKey::Name,
            sort_dir: SortDir::Asc,
        };
        s.refresh();
        s
    }

    /// Advance to the next sort key (cycles Name → Size → Type → Age → Name)
    /// and reset direction to ascending.
    pub(crate) fn cycle_sort_key(&mut self) {
        self.sort_key = self.sort_key.next();
        self.sort_dir = SortDir::Asc;
    }

    /// Flip the sort direction without changing the sort key.
    pub(crate) fn toggle_sort_dir(&mut self) {
        self.sort_dir = self.sort_dir.toggle();
    }

    /// Set a transient status message and record the current time for auto-clearing.
    pub(crate) fn set_status(&mut self, msg: String) {
        self.status_msg = Some(msg);
        self.status_msg_at = Some(Instant::now());
    }

    /// Clear the status message if it has been visible for more than `secs` seconds.
    pub(crate) fn tick_status(&mut self, secs: u64) {
        if let Some(at) = self.status_msg_at {
            if at.elapsed().as_secs() >= secs {
                self.status_msg = None;
                self.status_msg_at = None;
            }
        }
    }

    /// True while a status message is pending (drives the event-loop poll timeout).
    pub(crate) fn has_pending_status(&self) -> bool {
        self.status_msg_at.is_some()
    }

    /// Recompute derived lists from the index. Clamps cursors to valid ranges.
    pub(crate) fn refresh(&mut self) {
        let cp = self.current_path.clone();

        // Collect (uuid, entry) pairs in index insertion order (= age order).
        let pairs: Vec<(String, u64, u8)> = self
            .handle
            .entries_in_path(&cp)
            .into_iter()
            .map(|(uuid, e)| (uuid.to_string(), e.size, file_category(&e.name)))
            .collect();

        // Apply sort.
        let sort_key = self.sort_key;
        let sort_dir = self.sort_dir;
        let mut sorted = pairs;
        match sort_key {
            SortKey::Age => {
                // Insertion order is preserved above; just reverse for Desc.
                if sort_dir == SortDir::Desc { sorted.reverse(); }
            }
            _ => {
                let handle = &self.handle;
                sorted.sort_by(|(ua, sa, ca), (ub, sb, cb)| {
                    let ord = match sort_key {
                        SortKey::Name => {
                            let na = handle.index.entries.get(ua).map(|e| e.name.as_str()).unwrap_or("");
                            let nb = handle.index.entries.get(ub).map(|e| e.name.as_str()).unwrap_or("");
                            na.cmp(nb)
                        }
                        SortKey::Size => sa.cmp(sb),
                        SortKey::Type => ca.cmp(cb).then_with(|| {
                            let na = handle.index.entries.get(ua).map(|e| e.name.as_str()).unwrap_or("");
                            let nb = handle.index.entries.get(ub).map(|e| e.name.as_str()).unwrap_or("");
                            na.cmp(nb)
                        }),
                        SortKey::Age => unreachable!(),
                    };
                    if sort_dir == SortDir::Desc { ord.reverse() } else { ord }
                });
            }
        }
        self.file_uuids = sorted.into_iter().map(|(uuid, _, _)| uuid).collect();

        // Recompute folder lists, merging in any session-only extra folders.
        let mut all = collect_all_folders(&self.handle);
        for ef in &self.extra_folders {
            if !all.contains(ef) {
                all.push(ef.clone());
            }
        }
        all.sort();
        self.all_folders = all;

        // Derive immediate subfolders of current_path from all_folders.
        let prefix = if cp.is_empty() { String::new() } else { format!("{cp}/") };
        let mut seen_subs: std::collections::HashSet<String> = std::collections::HashSet::new();
        for path in &self.all_folders {
            if path.is_empty() { continue; }
            if !prefix.is_empty() && !path.starts_with(&prefix) { continue; }
            let rest = if prefix.is_empty() { path.as_str() } else { &path[prefix.len()..] };
            let seg = rest.split('/').next().unwrap_or("");
            if !seg.is_empty() { seen_subs.insert(seg.to_string()); }
        }
        self.folders = {
            let mut v: Vec<String> = seen_subs.into_iter().collect();
            v.sort();
            v
        };

        let list_len = self.folders.len() + self.file_uuids.len();
        if list_len == 0 {
            self.list_cursor = 0;
        } else if self.list_cursor >= list_len {
            self.list_cursor = list_len - 1;
        }

        // Keep tree cursor pointing at current_path
        if let Some(pos) = self.all_folders.iter().position(|f| f == &cp) {
            self.tree_cursor = pos;
        }
        let tree_len = self.all_folders.len();
        if tree_len > 0 && self.tree_cursor >= tree_len {
            self.tree_cursor = tree_len - 1;
        }
    }

    pub(crate) fn list_count(&self) -> usize {
        self.folders.len() + self.file_uuids.len()
    }

    pub(crate) fn move_list_up(&mut self) {
        if self.list_cursor > 0 { self.list_cursor -= 1; }
    }

    pub(crate) fn move_list_down(&mut self) {
        let max = self.list_count().saturating_sub(1);
        if self.list_cursor < max { self.list_cursor += 1; }
    }

    pub(crate) fn move_tree_up(&mut self) {
        if self.tree_cursor > 0 { self.tree_cursor -= 1; }
    }

    pub(crate) fn move_tree_down(&mut self) {
        let max = self.all_folders.len().saturating_sub(1);
        if self.tree_cursor < max { self.tree_cursor += 1; }
    }

    /// Navigate into a sub-folder by its bare name.
    pub(crate) fn navigate_into(&mut self, name: &str) {
        self.current_path = if self.current_path.is_empty() {
            name.to_string()
        } else {
            format!("{}/{name}", self.current_path)
        };
        self.selected_uuids.clear();
        self.list_cursor = 0;
        self.refresh();
    }

    /// Navigate to the path currently under the tree cursor.
    pub(crate) fn navigate_tree_cursor(&mut self) {
        if let Some(path) = self.all_folders.get(self.tree_cursor).cloned() {
            self.current_path = path;
            self.selected_uuids.clear();
            self.list_cursor = 0;
            self.refresh();
        }
    }

    /// Go up one folder level. Returns false if already at root.
    pub(crate) fn navigate_up(&mut self) -> bool {
        if self.current_path.is_empty() { return false; }
        self.current_path = match self.current_path.rfind('/') {
            Some(pos) => self.current_path[..pos].to_string(),
            None => String::new(),
        };
        self.selected_uuids.clear();
        self.list_cursor = 0;
        self.refresh();
        true
    }

    /// The folder name under the list cursor, if the cursor is on a folder row.
    pub(crate) fn cursor_folder(&self) -> Option<&str> {
        self.folders.get(self.list_cursor).map(String::as_str)
    }

    /// The file UUID under the list cursor, if the cursor is on a file row.
    pub(crate) fn cursor_file_uuid(&self) -> Option<&str> {
        let fi = self.list_cursor.checked_sub(self.folders.len())?;
        self.file_uuids.get(fi).map(String::as_str)
    }

    /// Effective target for operations: `selected_uuids` if non-empty, else the cursor file.
    pub(crate) fn effective_selection(&self) -> Vec<String> {
        if !self.selected_uuids.is_empty() {
            let mut v: Vec<String> = self.selected_uuids.iter().cloned().collect();
            v.sort();
            v
        } else if let Some(uuid) = self.cursor_file_uuid() {
            vec![uuid.to_string()]
        } else {
            vec![]
        }
    }

    /// Toggle selection for the current cursor file.
    pub(crate) fn toggle_selection(&mut self) {
        if let Some(uuid) = self.cursor_file_uuid().map(str::to_string) {
            if !self.selected_uuids.remove(&uuid) {
                self.selected_uuids.insert(uuid);
            }
        }
    }

    /// Get a reference to the entry for a UUID.
    pub(crate) fn entry(&self, uuid: &str) -> Option<&VaultEntry> {
        self.handle.index.entries.get(uuid)
    }

    /// Get the display name of a vault folder path (last segment, or "/" for root).
    pub(crate) fn folder_display_name(path: &str) -> &str {
        if path.is_empty() { return "/"; }
        match path.rfind('/') {
            Some(pos) => &path[pos + 1..],
            None => path,
        }
    }

    /// Depth of a folder path (0 = root, 1 = "photos", 2 = "photos/summer").
    pub(crate) fn folder_depth(path: &str) -> usize {
        if path.is_empty() { 0 } else { path.chars().filter(|&c| c == '/').count() + 1 }
    }
}

// ── Phase ──────────────────────────────────────────────────────────────────

pub(crate) enum Phase {
    /// Top-level submenu: Open Vault / New Vault.
    VaultMenu { cursor: usize },
    Locked {
        vault_path: String,
        password: String,
        focus: usize,
        /// `true` while the vault-path field is in keyboard-edit mode.
        path_edit_mode: bool,
        error: Option<String>,
    },
    /// Create-new-vault form (3 fields: vault folder, blobs subfolder, password).
    Creating {
        vault_path: String,
        blobs_dir: String,
        password: String,
        focus: usize,
        error: Option<String>,
    },
    /// Confirm creating a non-existent directory before writing the vault.
    ConfirmCreateDir {
        vault_path: String,
        blobs_dir: String,
        password: String,
    },
    /// Background PBKDF2 + index decrypt (or create) in progress.
    Opening(u8),
    /// Vault open, user is browsing. Browse data lives in `VaultState::browse`.
    Browse,
    /// Rename overlay on top of Browse.
    Rename { uuid: String, input: String },
    /// Delete confirmation overlay.
    ConfirmDelete { uuids: Vec<String> },
    /// Move-destination picker overlay.
    Move { uuids: Vec<String>, tree_cursor: usize },
    /// Background file-add operation in progress (overlay on top of Browse).
    Adding { total: usize, done: usize, current_file: String },
    /// New-folder dialog (overlay on top of Browse).
    NewFolder { parent: String, input: String, error: Option<String> },
    /// Background file-decrypt for preview in progress (overlay on top of Browse).
    Previewing { filename: String },
    /// Decrypted bytes ready; `render_vault_preview` must be called on the main thread.
    PreviewReady { bytes: Vec<u8>, ext: String },
    /// Background file-export (decrypt-to-disk) in progress (overlay on top of Browse).
    Exporting { total: usize, done: usize, current_file: String },
    /// Background decryption of folder images for the gallery in progress.
    LoadingGallery { folder: String, done: usize, total: usize },
    /// All gallery images decrypted; `render_vault_gallery` must be called on the main thread.
    GalleryReady { images: Vec<(String, Vec<u8>)> },
}

// ── Top-level state ────────────────────────────────────────────────────────

pub(crate) struct VaultState {
    pub(crate) phase: Phase,
    /// Present whenever the vault is unlocked (Browse + all overlay phases).
    pub(crate) browse: Option<BrowseState>,
    pub(super) rx: Option<mpsc::Receiver<WorkerMsg>>,
    /// Receiver for the background add-files worker.
    pub(super) add_rx: Option<mpsc::Receiver<AddWorkerMsg>>,
    /// Receiver for the background preview-decrypt worker.
    pub(super) preview_rx: Option<mpsc::Receiver<PreviewWorkerMsg>>,
    /// Receiver for the background export worker.
    pub(super) export_rx: Option<mpsc::Receiver<ExportWorkerMsg>>,
    /// Receiver for the background gallery-load worker.
    pub(super) gallery_rx: Option<mpsc::Receiver<GalleryWorkerMsg>>,
    /// UUIDs queued for export, stored while the file-browser is open to pick a dest dir.
    pub(crate) pending_export_uuids: Vec<String>,
}

impl VaultState {
    pub(crate) fn new() -> Self {
        VaultState {
            phase: Phase::VaultMenu { cursor: 0 },
            browse: None,
            rx: None,
            add_rx: None,
            preview_rx: None,
            export_rx: None,
            gallery_rx: None,
            pending_export_uuids: Vec::new(),
        }
    }

    /// Set the vault path from the file browser. Advances focus to password field.
    pub(crate) fn set_path(&mut self, path: &str) {
        if let Phase::Locked { vault_path, focus, .. } = &mut self.phase {
            *vault_path = path.to_string();
            *focus = 1;
        }
    }

    pub(crate) fn advance_focus(&mut self) {
        if let Phase::Locked { focus, .. } = &mut self.phase {
            *focus = (*focus + 1) % 2;
        }
    }

    pub(crate) fn is_opening(&self) -> bool {
        matches!(self.phase, Phase::Opening(_))
    }

    pub(crate) fn is_adding(&self) -> bool {
        matches!(self.phase, Phase::Adding { .. })
    }

    pub(crate) fn is_previewing(&self) -> bool {
        matches!(self.phase, Phase::Previewing { .. } | Phase::PreviewReady { .. })
    }

    pub(crate) fn is_exporting(&self) -> bool {
        matches!(self.phase, Phase::Exporting { .. })
    }

    // ── Preview ─────────────────────────────────────────────────────────────

    /// Decrypt a vault entry in a background thread and transition to `Previewing`.
    pub(crate) fn start_preview(&mut self, uuid: &str) {
        let browse = match &self.browse { Some(b) => b, None => return };
        let entry = match browse.handle.index.entries.get(uuid) {
            Some(e) => e.clone(),
            None => return,
        };
        let blobs_dir = browse.handle.blobs_dir.clone();
        let ext = std::path::Path::new(&entry.name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let filename = entry.name.clone();

        let (tx, rx) = mpsc::channel::<PreviewWorkerMsg>();
        self.preview_rx = Some(rx);
        self.phase = Phase::Previewing { filename };

        std::thread::spawn(move || {
            let result: Result<Vec<u8>, String> = (|| {
                let mut out = Vec::with_capacity(entry.size as usize);
                for part in &entry.parts {
                    let blob_path = blobs_dir.join(&part.uuid);
                    let blob = std::fs::read(&blob_path).map_err(|e| e.to_string())?;
                    let plain = super::crypto::decrypt_blob_with_key(&blob, &part.key_base64)
                        .map_err(|e| format!("{e:?}"))?;
                    out.extend_from_slice(&plain);
                }
                Ok(out)
            })();
            match result {
                Ok(bytes) => { let _ = tx.send(PreviewWorkerMsg::Ready(bytes, ext)); }
                Err(e)    => { let _ = tx.send(PreviewWorkerMsg::Failed(e)); }
            }
        });
    }

    /// Drain messages from the background preview worker.
    pub(crate) fn poll_preview_progress(&mut self) {
        if self.preview_rx.is_none() { return; }
        loop {
            match self.preview_rx.as_ref().unwrap().try_recv() {
                Ok(PreviewWorkerMsg::Ready(bytes, ext)) => {
                    self.preview_rx = None;
                    self.phase = Phase::PreviewReady { bytes, ext };
                    break;
                }
                Ok(PreviewWorkerMsg::Failed(msg)) => {
                    self.preview_rx = None;
                    if let Some(b) = &mut self.browse {
                        b.set_status(format!("Preview failed: {msg}"));
                    }
                    self.phase = Phase::Browse;
                    break;
                }
                Err(_) => break,
            }
        }
    }

    // ── Export ──────────────────────────────────────────────────────────────

    /// Decrypt the entries in `pending_export_uuids` to `dest_dir` in a background thread.
    pub(crate) fn start_export(&mut self, dest_dir: PathBuf) {
        let uuids = std::mem::take(&mut self.pending_export_uuids);
        if uuids.is_empty() { return; }
        let browse = match &self.browse { Some(b) => b, None => return };

        let entries: Vec<(String, super::types::VaultEntry)> = uuids.iter()
            .filter_map(|u| browse.handle.index.entries.get(u).map(|e| (u.clone(), e.clone())))
            .collect();
        let blobs_dir = browse.handle.blobs_dir.clone();
        let total = entries.len();

        let (tx, rx) = mpsc::channel::<ExportWorkerMsg>();
        self.export_rx = Some(rx);
        self.phase = Phase::Exporting { total, done: 0, current_file: String::new() };

        std::thread::spawn(move || {
            let mut exported = 0usize;
            for (i, (_uuid, entry)) in entries.iter().enumerate() {
                let _ = tx.send(ExportWorkerMsg::Progress {
                    done: i,
                    total,
                    filename: entry.name.clone(),
                });
                let result: Result<(), String> = (|| {
                    let mut data = Vec::with_capacity(entry.size as usize);
                    for part in &entry.parts {
                        let blob_path = blobs_dir.join(&part.uuid);
                        let blob = std::fs::read(&blob_path).map_err(|e| e.to_string())?;
                        let plain = super::crypto::decrypt_blob_with_key(&blob, &part.key_base64)
                            .map_err(|e| format!("{e:?}"))?;
                        data.extend_from_slice(&plain);
                    }
                    let out_path = dest_dir.join(&entry.name);
                    std::fs::write(&out_path, &data).map_err(|e| e.to_string())
                })();
                match result {
                    Ok(()) => { exported += 1; }
                    Err(e) => {
                        let _ = tx.send(ExportWorkerMsg::Failed(
                            format!("{}: {e}", entry.name)
                        ));
                        return;
                    }
                }
            }
            let _ = tx.send(ExportWorkerMsg::Done(exported));
        });
    }

    /// Drain messages from the background export worker.
    pub(crate) fn poll_export_progress(&mut self) {
        if self.export_rx.is_none() { return; }
        loop {
            match self.export_rx.as_ref().unwrap().try_recv() {
                Ok(ExportWorkerMsg::Progress { done, total, filename }) => {
                    self.phase = Phase::Exporting { done, total, current_file: filename };
                }
                Ok(ExportWorkerMsg::Done(n)) => {
                    self.export_rx = None;
                    if let Some(b) = &mut self.browse {
                        b.set_status(format!("Exported {n} file(s)"));
                    }
                    self.phase = Phase::Browse;
                    break;
                }
                Ok(ExportWorkerMsg::Failed(msg)) => {
                    self.export_rx = None;
                    if let Some(b) = &mut self.browse {
                        b.set_status(format!("Export failed: {msg}"));
                    }
                    self.phase = Phase::Browse;
                    break;
                }
                Err(_) => break,
            }
        }
    }

    // ── Gallery ─────────────────────────────────────────────────────────────

    pub(crate) fn is_loading_gallery(&self) -> bool {
        matches!(self.phase, Phase::LoadingGallery { .. })
    }

    /// Collect all image entries recursively under `folder` and decrypt them in a
    /// background thread. Transitions to `LoadingGallery` while running, then
    /// `GalleryReady` when all images are in memory.
    pub(crate) fn start_folder_gallery(&mut self, folder: &str) {
        let browse = match &self.browse { Some(b) => b, None => return };

        fn is_img(name: &str) -> bool {
            let lower = name.to_ascii_lowercase();
            let ext = std::path::Path::new(&lower)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "tif")
        }

        // Collect (name, size, category, parts) for every image entry under `folder`.
        let folder_str = folder.to_string();
        let mut items: Vec<(String, u64, u8, Vec<(String, String)>)> = browse.handle.index.entries
            .values()
            .filter(|e| {
                let in_folder = if folder_str.is_empty() {
                    true
                } else {
                    e.path == folder_str || e.path.starts_with(&format!("{folder_str}/"))
                };
                in_folder && is_img(&e.name)
            })
            .map(|e| (
                e.name.clone(),
                e.size,
                file_category(&e.name),
                e.parts.iter().map(|p| (p.uuid.clone(), p.key_base64.clone())).collect(),
            ))
            .collect();

        let total = items.len();
        if total == 0 {
            if let Some(b) = &mut self.browse {
                b.set_status("No images found in folder".to_string());
            }
            return;
        }

        // Sort items in the same order as the current file list.
        let sort_key = browse.sort_key;
        let sort_dir = browse.sort_dir;
        match sort_key {
            SortKey::Age => {
                // items are already in IndexMap insertion order (age order)
                if sort_dir == SortDir::Desc { items.reverse(); }
            }
            SortKey::Name => {
                items.sort_by(|(na, _, _, _), (nb, _, _, _)| {
                    let ord = na.cmp(nb);
                    if sort_dir == SortDir::Desc { ord.reverse() } else { ord }
                });
            }
            SortKey::Size => {
                items.sort_by(|(_, sa, _, _), (_, sb, _, _)| {
                    let ord = sa.cmp(sb);
                    if sort_dir == SortDir::Desc { ord.reverse() } else { ord }
                });
            }
            SortKey::Type => {
                items.sort_by(|(na, _, ca, _), (nb, _, cb, _)| {
                    let ord = ca.cmp(cb).then_with(|| na.cmp(nb));
                    if sort_dir == SortDir::Desc { ord.reverse() } else { ord }
                });
            }
        }

        let blobs_dir = browse.handle.blobs_dir.clone();

        let (tx, rx) = mpsc::channel::<GalleryWorkerMsg>();
        self.gallery_rx = Some(rx);
        self.phase = Phase::LoadingGallery { folder: folder.to_string(), done: 0, total };

        std::thread::spawn(move || {
            let mut images: Vec<(String, Vec<u8>)> = Vec::with_capacity(total);
            for (done, (name, _size, _cat, parts)) in items.into_iter().enumerate() {
                let _ = tx.send(GalleryWorkerMsg::Progress { done, total });
                let result: Result<Vec<u8>, String> = (|| {
                    let mut out = Vec::new();
                    for (uuid, key_b64) in &parts {
                        let blob = std::fs::read(blobs_dir.join(uuid))
                            .map_err(|e| e.to_string())?;
                        let plain = super::crypto::decrypt_blob_with_key(&blob, key_b64)
                            .map_err(|e| format!("{e:?}"))?;
                        out.extend_from_slice(&plain);
                    }
                    Ok(out)
                })();
                if let Ok(bytes) = result {
                    images.push((name, bytes));
                }
                // Silently skip images that fail to decrypt.
            }
            // Items were pre-sorted; no additional sort needed.
            let _ = tx.send(GalleryWorkerMsg::Ready(images));
        });
    }

    /// Start the gallery for the folder currently selected in the tree panel.
    pub(crate) fn start_gallery_for_tree_cursor(&mut self) {
        let folder = match &self.browse {
            Some(b) => b.all_folders.get(b.tree_cursor).cloned().unwrap_or_default(),
            None => return,
        };
        self.start_folder_gallery(&folder);
    }

    /// Start the gallery for the folder currently being browsed (current_path).
    pub(crate) fn start_gallery_for_current_path(&mut self) {
        let folder = match &self.browse {
            Some(b) => b.current_path.clone(),
            None => return,
        };
        self.start_folder_gallery(&folder);
    }

    /// Drain messages from the background gallery worker.
    pub(crate) fn poll_gallery_progress(&mut self) {
        if self.gallery_rx.is_none() { return; }
        loop {
            match self.gallery_rx.as_ref().unwrap().try_recv() {
                Ok(GalleryWorkerMsg::Progress { done, total }) => {
                    if let Phase::LoadingGallery { done: d, .. } = &mut self.phase {
                        *d = done;
                        let _ = total; // already in phase
                    }
                }
                Ok(GalleryWorkerMsg::Ready(images)) => {
                    self.gallery_rx = None;
                    self.phase = Phase::GalleryReady { images };
                    break;
                }
                Ok(GalleryWorkerMsg::Failed(msg)) => {
                    self.gallery_rx = None;
                    if let Some(b) = &mut self.browse {
                        b.set_status(format!("Gallery failed: {msg}"));
                    }
                    self.phase = Phase::Browse;
                    break;
                }
                Err(_) => break,
            }
        }
    }

    // ── Creating ────────────────────────────────────────────────────────────

    pub(crate) fn enter_creating(&mut self) {
        self.phase = Phase::Creating {
            vault_path: String::new(),
            blobs_dir: String::new(),
            password: String::new(),
            focus: 0,
            error: None,
        };
    }

    /// Set vault_path from the directory browser during the Creating flow. Advances focus to blobs_dir.
    pub(crate) fn set_create_path(&mut self, path: &str) {
        if let Phase::Creating { vault_path, focus, .. } = &mut self.phase {
            *vault_path = path.to_string();
            *focus = 1;
        }
    }

    pub(crate) fn advance_create_focus(&mut self) {
        if let Phase::Creating { focus, .. } = &mut self.phase {
            *focus = (*focus + 1) % 3;
        }
    }

    fn set_creating_error(&mut self, msg: &str) {
        if let Phase::Creating { error, .. } = &mut self.phase {
            *error = Some(msg.to_string());
        }
    }

    /// Validate inputs and spawn the background create thread.
    pub(crate) fn start_create(&mut self) {
        let (vault_path, blobs_dir, password) = match &self.phase {
            Phase::Creating { vault_path, blobs_dir, password, .. } => {
                (vault_path.clone(), blobs_dir.clone(), password.clone())
            }
            _ => return,
        };

        if vault_path.is_empty() {
            self.set_creating_error("Vault folder cannot be empty.");
            return;
        }
        if password.is_empty() {
            self.set_creating_error("Password cannot be empty.");
            return;
        }

        let p = std::path::Path::new(&vault_path);
        if p.is_file() {
            self.set_creating_error("Path points to a file, not a directory.");
            return;
        }
        if !p.exists() {
            self.phase = Phase::ConfirmCreateDir { vault_path, blobs_dir, password };
            return;
        }

        self.spawn_create(vault_path, blobs_dir, password);
    }

    /// Confirm creating the missing directory and proceed with vault creation.
    pub(crate) fn confirm_create_dir(&mut self) {
        let (vault_path, blobs_dir, password) = match &self.phase {
            Phase::ConfirmCreateDir { vault_path, blobs_dir, password } => {
                (vault_path.clone(), blobs_dir.clone(), password.clone())
            }
            _ => return,
        };
        self.spawn_create(vault_path, blobs_dir, password);
    }

    /// Cancel directory creation and return to the Creating form.
    pub(crate) fn cancel_create_dir(&mut self) {
        let (vault_path, blobs_dir, password) = match &self.phase {
            Phase::ConfirmCreateDir { vault_path, blobs_dir, password } => {
                (vault_path.clone(), blobs_dir.clone(), password.clone())
            }
            _ => return,
        };
        self.phase = Phase::Creating { vault_path, blobs_dir, password, focus: 0, error: None };
    }

    fn spawn_create(&mut self, vault_path: String, blobs_dir: String, password: String) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.phase = Phase::Opening(0);

        std::thread::spawn(move || {
            let _ = tx.send(WorkerMsg::Progress(5));
            let path = std::path::Path::new(&vault_path);
            // Create the directory if it doesn't exist yet (confirmed by user).
            if !path.exists() {
                if let Err(e) = std::fs::create_dir_all(path) {
                    let _ = tx.send(WorkerMsg::Failed(format!("Could not create directory: {e}")));
                    return;
                }
            }
            let blobs = if blobs_dir.is_empty() { None } else { Some(blobs_dir.as_str()) };
            match super::crypto::create_vault(path, blobs, &password) {
                Ok(handle) => { let _ = tx.send(WorkerMsg::Created(handle)); }
                Err(e)     => { let _ = tx.send(WorkerMsg::Failed(e.to_string())); }
            }
        });
    }

    /// Validate inputs and spawn the background unlock thread.
    pub(crate) fn start_unlock(&mut self) {
        let (vault_path, password) = match &self.phase {
            Phase::Locked { vault_path, password, .. } => (vault_path.clone(), password.clone()),
            _ => return,
        };

        if vault_path.is_empty() {
            self.set_locked_error("Vault path cannot be empty.");
            return;
        }
        if password.is_empty() {
            self.set_locked_error("Password cannot be empty.");
            return;
        }

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.phase = Phase::Opening(0);

        std::thread::spawn(move || {
            let _ = tx.send(WorkerMsg::Progress(5));
            let path = std::path::Path::new(&vault_path);
            match super::crypto::open_vault(path, &password) {
                Ok(handle) => { let _ = tx.send(WorkerMsg::Opened(handle)); }
                Err(e)    => { let _ = tx.send(WorkerMsg::Failed(e.to_string())); }
            }
        });
    }

    fn set_locked_error(&mut self, msg: &str) {
        if let Phase::Locked { error, .. } = &mut self.phase {
            *error = Some(msg.to_string());
        }
    }

    /// Drain worker channel; transitions Opening → Browse or back to Locked on error.
    pub(crate) fn poll_progress(&mut self) {
        if self.rx.is_none() { return; }
        loop {
            match self.rx.as_ref().unwrap().try_recv() {
                Ok(WorkerMsg::Progress(pct)) => {
                    self.phase = Phase::Opening(pct);
                }
                Ok(WorkerMsg::Opened(handle)) | Ok(WorkerMsg::Created(handle)) => {
                    self.rx = None;
                    self.browse = Some(BrowseState::new(handle));
                    self.phase = Phase::Browse;
                    break;
                }
                Ok(WorkerMsg::Failed(msg)) => {
                    self.rx = None;
                    self.phase = Phase::Locked {
                        vault_path: String::new(),
                        password: String::new(),
                        focus: 0,
                        path_edit_mode: false,
                        error: Some(msg),
                    };
                    break;
                }
                Err(_) => break,
            }
        }
    }

    // ── Operations ──────────────────────────────────────────────────────────

    /// Enter rename overlay for the single selected/cursor file.
    pub(crate) fn enter_rename(&mut self) {
        let browse = match &self.browse { Some(b) => b, None => return };
        let sel = browse.effective_selection();
        if sel.len() != 1 { return; }
        let uuid = sel[0].clone();
        if let Some(entry) = browse.entry(&uuid) {
            let input = entry.name.clone();
            self.phase = Phase::Rename { uuid, input };
        }
    }

    /// Confirm the current rename input. Returns to Browse.
    pub(crate) fn confirm_rename(&mut self) {
        let (uuid, new_name) = match &self.phase {
            Phase::Rename { uuid, input } => (uuid.clone(), input.clone()),
            _ => return,
        };
        let browse = match &mut self.browse { Some(b) => b, None => return };
        let cp = browse.current_path.clone();

        let conflict = browse.handle.entries_in_path(&cp)
            .into_iter()
            .any(|(u, e)| u != uuid.as_str() && e.name == new_name);

        if conflict {
            browse.set_status(format!("'{new_name}' already exists in this folder"));
        } else if !new_name.is_empty() {
            if let Some(entry) = browse.handle.index.entries.get_mut(&uuid) {
                entry.name = new_name;
            }
            browse.dirty = true;
            browse.refresh();
        }
        self.phase = Phase::Browse;
    }

    /// Enter delete confirmation overlay.
    pub(crate) fn enter_delete(&mut self) {
        let browse = match &self.browse { Some(b) => b, None => return };
        let uuids = browse.effective_selection();
        if uuids.is_empty() { return; }
        self.phase = Phase::ConfirmDelete { uuids };
    }

    /// Execute deletion and auto-save. Returns to Browse.
    pub(crate) fn confirm_delete(&mut self) {
        let uuids = match &self.phase {
            Phase::ConfirmDelete { uuids } => uuids.clone(),
            _ => return,
        };
        let browse = match &mut self.browse { Some(b) => b, None => return };

        // Collect all blob UUIDs (parts + thumbnails) before removing index entries.
        let mut blob_paths: Vec<std::path::PathBuf> = Vec::new();
        for uuid in &uuids {
            if let Some(entry) = browse.handle.index.entries.get(uuid) {
                for part in &entry.parts {
                    blob_paths.push(browse.handle.blob_path(&part.uuid));
                }
                if let Some(thumb_uuid) = &entry.thumbnail_uuid {
                    blob_paths.push(browse.handle.blob_path(thumb_uuid));
                }
            }
        }

        for uuid in &uuids {
            browse.handle.index.entries.shift_remove(uuid);
            browse.selected_uuids.remove(uuid);
            browse.clipboard.retain(|u| u != uuid);
        }

        // Delete blob files from disk (best-effort — ignore individual failures).
        for path in &blob_paths {
            let _ = std::fs::remove_file(path);
        }

        let n = uuids.len();
        let msg = match super::crypto::save_vault(&browse.handle) {
            Ok(()) => { browse.dirty = false; format!("Deleted {n} item(s) — saved") }
            Err(e) => { browse.dirty = true;  format!("Deleted {n} item(s) — save failed: {e}") }
        };
        browse.set_status(msg);
        browse.refresh();
        self.phase = Phase::Browse;
    }

    /// Cut effective selection into the clipboard.
    pub(crate) fn cut_selection(&mut self) {
        let browse = match &mut self.browse { Some(b) => b, None => return };
        let sel = browse.effective_selection();
        if sel.is_empty() { return; }
        browse.clipboard = sel;
        browse.selected_uuids.clear();
        browse.set_status(format!("{} item(s) cut — press p to paste", browse.clipboard.len()));
    }

    /// Move clipboard items to the current path and auto-save.
    pub(crate) fn paste(&mut self) {
        let browse = match &mut self.browse { Some(b) => b, None => return };
        let uuids = browse.clipboard.clone();
        if uuids.is_empty() {
            browse.set_status("Nothing in clipboard".to_string());
            return;
        }
        let dest = browse.current_path.clone();

        // Compute new names first (immutable pass), then apply (mutable pass)
        let mut resolved: Vec<(String, String)> = Vec::new(); // (uuid, final_name)
        for uuid in &uuids {
            let base = browse.handle.index.entries.get(uuid)
                .map(|e| e.name.clone())
                .unwrap_or_default();
            let mut final_name = base.clone();
            let mut counter = 1u32;
            loop {
                let conflict = browse.handle.index.entries.iter()
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
        for (uuid, final_name) in resolved {
            if let Some(entry) = browse.handle.index.entries.get_mut(&uuid) {
                entry.name = final_name;
                entry.path = dest.clone();
            }
        }
        browse.clipboard.clear();
        let n = uuids.len();
        let msg = match super::crypto::save_vault(&browse.handle) {
            Ok(()) => { browse.dirty = false; format!("Moved {n} item(s) — saved") }
            Err(e) => { browse.dirty = true;  format!("Moved {n} item(s) — save failed: {e}") }
        };
        browse.set_status(msg);
        browse.refresh();
    }

    /// Enter the move-destination picker overlay.
    pub(crate) fn enter_move(&mut self) {
        let browse = match &self.browse { Some(b) => b, None => return };
        let uuids = browse.effective_selection();
        if uuids.is_empty() { return; }
        // Start tree cursor at the current path
        let tree_cursor = browse.all_folders
            .iter()
            .position(|f| f == &browse.current_path)
            .unwrap_or(0);
        self.phase = Phase::Move { uuids, tree_cursor };
    }

    /// Move items to the folder at the move overlay's tree cursor. Auto-saves.
    pub(crate) fn confirm_move(&mut self) {
        let (uuids, tree_cursor) = match &self.phase {
            Phase::Move { uuids, tree_cursor } => (uuids.clone(), *tree_cursor),
            _ => return,
        };
        let browse = match &mut self.browse { Some(b) => b, None => return };
        let dest = browse.all_folders.get(tree_cursor).cloned().unwrap_or_default();

        // Resolve names without conflicts — immutable pass (same pattern as paste())
        let mut resolved: Vec<(String, String)> = Vec::new();
        for uuid in &uuids {
            let base = browse.handle.index.entries.get(uuid)
                .map(|e| e.name.clone())
                .unwrap_or_default();
            let mut final_name = base.clone();
            let mut counter = 1u32;
            loop {
                let conflict = browse.handle.index.entries.iter()
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

        // Apply — mutable pass
        for (uuid, final_name) in resolved {
            if let Some(entry) = browse.handle.index.entries.get_mut(&uuid) {
                entry.name = final_name;
                entry.path = dest.clone();
            }
        }
        browse.selected_uuids.clear();
        let n = uuids.len();
        let msg = match super::crypto::save_vault(&browse.handle) {
            Ok(()) => { browse.dirty = false; format!("Moved {n} item(s) — saved") }
            Err(e) => { browse.dirty = true;  format!("Moved {n} item(s) — save failed: {e}") }
        };
        browse.set_status(msg);
        browse.refresh();
        self.phase = Phase::Browse;
    }

    /// Explicitly save the index.
    pub(crate) fn save(&mut self) {
        let browse = match &mut self.browse { Some(b) => b, None => return };
        let msg = match super::crypto::save_vault(&browse.handle) {
            Ok(()) => { browse.dirty = false; "Vault saved.".to_string() }
            Err(e) => format!("Save failed: {e}"),
        };
        browse.set_status(msg);
    }

    // ── Create folder ────────────────────────────────────────────────────────

    /// Open the new-folder dialog as a child of the current path.
    pub(crate) fn enter_new_folder(&mut self) {
        let parent = match &self.browse {
            Some(b) => b.current_path.clone(),
            None => return,
        };
        self.phase = Phase::NewFolder { parent, input: String::new(), error: None };
    }

    /// Validate the typed name and append the new folder to `extra_folders`.
    pub(crate) fn confirm_new_folder(&mut self) {
        let (parent, input) = match &self.phase {
            Phase::NewFolder { parent, input, .. } => (parent.clone(), input.clone()),
            _ => return,
        };

        let name = input.trim().to_string();

        if name.is_empty() {
            if let Phase::NewFolder { error, .. } = &mut self.phase {
                *error = Some("Folder name cannot be empty.".into());
            }
            return;
        }
        if name.contains('/') {
            if let Phase::NewFolder { error, .. } = &mut self.phase {
                *error = Some("Folder name cannot contain '/'.".into());
            }
            return;
        }

        let full_path = if parent.is_empty() {
            name.clone()
        } else {
            format!("{parent}/{name}")
        };

        let browse = match &mut self.browse { Some(b) => b, None => { self.phase = Phase::Browse; return; } };

        if browse.all_folders.contains(&full_path) {
            if let Phase::NewFolder { error, .. } = &mut self.phase {
                *error = Some(format!("A folder named '{name}' already exists here."));
            }
            return;
        }

        browse.extra_folders.push(full_path.clone());
        browse.extra_folders.sort();
        browse.set_status(format!("Folder '{name}' created — move files here with m or x/p"));
        browse.refresh();
        self.phase = Phase::Browse;
    }

    // ── Add files ───────────────────────────────────────────────────────────

    /// Spawn a background thread to encrypt and add `paths` to the vault.
    /// Transitions to `Phase::Adding`. Ignores the call if no vault is open.
    pub(crate) fn start_add(&mut self, paths: Vec<PathBuf>) {
        let browse = match &self.browse { Some(b) => b, None => return };
        if paths.is_empty() { return; }

        let blobs_dir = browse.handle.blobs_dir.clone();
        let virtual_path = browse.current_path.clone();
        let total = paths.len();

        let (tx, rx) = mpsc::channel();
        self.add_rx = Some(rx);
        self.phase = Phase::Adding { total, done: 0, current_file: String::new() };

        std::thread::spawn(move || {
            let mut results: Vec<(String, super::types::VaultEntry)> = Vec::new();

            for (i, file_path) in paths.iter().enumerate() {
                let filename = file_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file_path.to_string_lossy().into_owned());

                let _ = tx.send(AddWorkerMsg::Progress {
                    done: i,
                    total,
                    filename: filename.clone(),
                });

                match super::crypto::encrypt_file_to_vault(file_path, &blobs_dir, &virtual_path) {
                    Ok(pair) => results.push(pair),
                    Err(e) => {
                        let _ = tx.send(AddWorkerMsg::Failed(
                            format!("Failed to add '{}': {}", filename, e)
                        ));
                        return;
                    }
                }
            }

            let _ = tx.send(AddWorkerMsg::Done(results));
        });
    }

    /// Drain the add-worker channel. Merges results into the index and auto-saves
    /// when complete. Should be called every frame from the main loop.
    pub(crate) fn poll_add_progress(&mut self) {
        if self.add_rx.is_none() { return; }
        loop {
            match self.add_rx.as_ref().unwrap().try_recv() {
                Ok(AddWorkerMsg::Progress { done, total, filename }) => {
                    self.phase = Phase::Adding { total, done, current_file: filename };
                }
                Ok(AddWorkerMsg::Done(entries)) => {
                    self.add_rx = None;
                    let browse = match &mut self.browse { Some(b) => b, None => { self.phase = Phase::Browse; break; } };
                    let n = entries.len();
                    for (uuid, entry) in entries {
                        browse.handle.index.entries.insert(uuid, entry);
                    }
                    let msg = match super::crypto::save_vault(&browse.handle) {
                        Ok(()) => { browse.dirty = false; format!("Added {n} file(s) — saved") }
                        Err(e) => { browse.dirty = true;  format!("Added {n} file(s) — save failed: {e}") }
                    };
                    browse.set_status(msg);
                    browse.refresh();
                    self.phase = Phase::Browse;
                    break;
                }
                Ok(AddWorkerMsg::Failed(msg)) => {
                    self.add_rx = None;
                    if let Some(browse) = &mut self.browse {
                        browse.set_status(msg);
                    }
                    self.phase = Phase::Browse;
                    break;
                }
                Err(_) => break,
            }
        }
    }

    /// Clear any status message that has been showing for ≥ `secs` seconds.
    /// Called each event-loop tick so the hint bar reappears automatically.
    pub(crate) fn tick(&mut self, secs: u64) {
        if let Some(b) = &mut self.browse {
            b.tick_status(secs);
        }
    }

    /// True if a timed status message is currently pending.
    pub(crate) fn has_pending_status(&self) -> bool {
        self.browse.as_ref().map(|b| b.has_pending_status()).unwrap_or(false)
    }

    /// Return to the vault submenu and discard the open vault.
    pub(crate) fn lock(&mut self) {
        self.browse = None;
        self.phase = Phase::VaultMenu { cursor: 0 };
    }

    // ── VaultMenu ───────────────────────────────────────────────────────────

    pub(crate) fn menu_up(&mut self) {
        if let Phase::VaultMenu { cursor } = &mut self.phase {
            if *cursor > 0 { *cursor -= 1; }
        }
    }

    pub(crate) fn menu_down(&mut self) {
        if let Phase::VaultMenu { cursor } = &mut self.phase {
            if *cursor < 1 { *cursor += 1; }
        }
    }

    /// Select the highlighted menu item (0 = Open, 1 = New).
    pub(crate) fn menu_select(&mut self) {
        let cursor = match &self.phase {
            Phase::VaultMenu { cursor } => *cursor,
            _ => return,
        };
        match cursor {
            0 => {
                self.phase = Phase::Locked {
                    vault_path: String::new(),
                    password: String::new(),
                    focus: 0,
                    path_edit_mode: false,
                    error: None,
                };
            }
            _ => self.enter_creating(),
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Collect all unique non-empty folder paths implied by the index, plus root "".
fn collect_all_folders(handle: &VaultHandle) -> Vec<String> {
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

fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(pos) if pos > 0 => (&name[..pos], &name[pos + 1..]),
        _ => (name, ""),
    }
}
