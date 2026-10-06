//! Filesystem model: entries, sorting, kinds, and Finder-style formatting.

use std::{
    cmp::Ordering,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use chrono::{DateTime, Datelike, Local};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconKind {
    Folder,
    App,
    Plain,
    Data,
    Image,
    Archive,
    Media,
    Text,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    /// A directory the user can expand / enter (application bundles are not).
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub kind: String,
    pub icon: IconKind,
    pub is_image: bool,
}

impl Entry {
    pub fn from_path(path: &Path) -> Option<Entry> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".to_string());
        Self::with_name(path, name)
    }

    pub fn with_name(path: &Path, name: String) -> Option<Entry> {
        // Follow symlinks so a link to a folder behaves like a folder.
        let meta = fs::metadata(path)
            .or_else(|_| fs::symlink_metadata(path))
            .ok()?;
        Some(Self::from_metadata(path, name, &meta))
    }

    fn from_metadata(path: &Path, name: String, meta: &fs::Metadata) -> Entry {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let is_app = meta.is_dir() && ext == "app";
        let is_dir = meta.is_dir() && !is_app;
        let (kind, icon) = if is_app {
            ("Application".to_string(), IconKind::App)
        } else if is_dir {
            ("Folder".to_string(), IconKind::Folder)
        } else {
            kind_for_extension(&ext)
        };
        Entry {
            path: path.to_path_buf(),
            name,
            is_dir,
            size: if meta.is_dir() {
                None
            } else {
                Some(meta.len())
            },
            modified: meta.modified().ok(),
            created: meta.created().ok(),
            accessed: meta.accessed().ok(),
            kind,
            icon,
            is_image: !meta.is_dir()
                && matches!(
                    ext.as_str(),
                    "png"
                        | "jpg"
                        | "jpeg"
                        | "gif"
                        | "webp"
                        | "bmp"
                        | "ico"
                        | "tiff"
                        | "tif"
                        | "svg"
                ),
        }
    }
}

fn kind_for_extension(ext: &str) -> (String, IconKind) {
    use IconKind::*;
    let (kind, icon): (&str, IconKind) = match ext {
        "" => ("Document", Plain),
        "png" => ("PNG image", Image),
        "jpg" | "jpeg" => ("JPEG image", Image),
        "gif" => ("GIF image", Image),
        "webp" => ("WebP image", Image),
        "bmp" => ("BMP image", Image),
        "ico" => ("Windows icon image", Image),
        "tif" | "tiff" => ("TIFF image", Image),
        "heic" => ("HEIC image", Image),
        "svg" => ("SVG image", Plain),
        "psd" => ("Adobe Photoshop document", Image),
        "json" => ("JSON Document", Data),
        "yaml" | "yml" => ("YAML Document", Data),
        "toml" => ("TOML Document", Data),
        "xml" => ("XML Document", Data),
        "plist" => ("Property List", Data),
        "csv" => ("CSV Document", Data),
        "txt" => ("Plain Text Document", Text),
        "md" | "markdown" => ("Markdown Document", Text),
        "rtf" => ("Rich Text Document", Text),
        "pdf" => ("PDF Document", Text),
        "doc" | "docx" => ("Microsoft Word document", Text),
        "xls" | "xlsx" => ("Microsoft Excel spreadsheet", Data),
        "ppt" | "pptx" => ("Microsoft PowerPoint presentation", Plain),
        "pages" => ("Pages Document", Text),
        "numbers" => ("Numbers Spreadsheet", Data),
        "key" => ("Keynote Presentation", Plain),
        "rs" => ("Rust Source", Data),
        "swift" => ("Swift Source", Data),
        "c" => ("C Source", Data),
        "h" => ("C Header", Data),
        "cpp" | "cc" | "cxx" => ("C++ Source", Data),
        "m" => ("Objective-C Source", Data),
        "js" | "mjs" => ("JavaScript script", Data),
        "ts" | "tsx" => ("TypeScript Source", Data),
        "jsx" => ("JavaScript XML", Data),
        "py" => ("Python script", Data),
        "rb" => ("Ruby script", Data),
        "go" => ("Go Source", Data),
        "java" => ("Java Source", Data),
        "sh" | "zsh" | "bash" => ("Shell script", Data),
        "html" | "htm" => ("HTML text", Data),
        "css" => ("CSS style sheet", Data),
        "wgsl" => ("WGSL Source", Data),
        "lock" => ("Lock File", Plain),
        "log" => ("Log File", Text),
        "zip" => ("ZIP archive", Archive),
        "gz" | "tgz" => ("GZIP archive", Archive),
        "tar" => ("TAR archive", Archive),
        "7z" => ("7-zip archive", Archive),
        "rar" => ("RAR archive", Archive),
        "dmg" => ("Disk Image", Archive),
        "pkg" => ("Installer Package", Archive),
        "mp3" => ("MP3 audio", Media),
        "m4a" => ("MPEG-4 audio", Media),
        "wav" => ("WAVE audio", Media),
        "flac" => ("FLAC audio", Media),
        "aac" => ("AAC audio", Media),
        "mp4" => ("MPEG-4 movie", Media),
        "m4v" => ("MPEG-4 video", Media),
        "mov" => ("QuickTime movie", Media),
        "mkv" => ("Matroska video", Media),
        "avi" => ("AVI movie", Media),
        "ttf" => ("TrueType font", Plain),
        "otf" => ("OpenType font", Plain),
        _ => {
            return (format!("{} document", ext.to_uppercase()), Plain);
        }
    };
    (kind.to_string(), icon)
}

/// A loaded directory listing.
pub struct Listing {
    pub entries: Vec<Arc<Entry>>,
    pub mtime: Option<SystemTime>,
}

pub fn read_dir(path: &Path, show_hidden: bool) -> io::Result<Listing> {
    let mtime = fs::metadata(path).and_then(|m| m.modified()).ok();
    let mut entries = Vec::new();
    for item in fs::read_dir(path)? {
        let Ok(item) = item else { continue };
        let name = item.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        if let Some(entry) = Entry::with_name(&item.path(), name) {
            entries.push(Arc::new(entry));
        }
    }
    Ok(Listing { entries, mtime })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Modified,
    Size,
    Kind,
}

pub fn sort_entries(entries: &mut [Arc<Entry>], key: SortKey, ascending: bool) {
    entries.sort_by(|a, b| {
        let ord = match key {
            SortKey::Name => natural_cmp(&a.name, &b.name),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            SortKey::Kind => natural_cmp(&a.kind, &b.kind),
        }
        .then_with(|| natural_cmp(&a.name, &b.name));
        if ascending { ord } else { ord.reverse() }
    });
}

/// Case-insensitive comparison that orders digit runs numerically ("2" < "10").
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut dx = String::new();
                while let Some(c) = ai.next_if(|c| c.is_ascii_digit()) {
                    dx.push(c);
                }
                let mut dy = String::new();
                while let Some(c) = bi.next_if(|c| c.is_ascii_digit()) {
                    dy.push(c);
                }
                let (tx, ty) = (dx.trim_start_matches('0'), dy.trim_start_matches('0'));
                let ord = tx.len().cmp(&ty.len()).then_with(|| tx.cmp(ty));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                ai.next();
                bi.next();
                let (lx, ly) = (
                    x.to_lowercase().next().unwrap_or(x),
                    y.to_lowercase().next().unwrap_or(y),
                );
                if lx != ly {
                    return lx.cmp(&ly);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Finder uses decimal (1000-based) units.
pub fn format_size(bytes: u64) -> String {
    if bytes == 1 {
        return "1 byte".into();
    }
    if bytes < 1000 {
        return format!("{bytes} bytes");
    }
    let units = ["KB", "MB", "GB", "TB"];
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 || value >= 10.0 {
        format!("{} {}", value.round() as u64, units[unit])
    } else {
        let s = format!("{value:.1}");
        format!("{} {}", s.trim_end_matches(".0"), units[unit])
    }
}

pub fn format_date_short(time: Option<SystemTime>) -> String {
    match time {
        Some(t) => DateTime::<Local>::from(t)
            .format("%-m/%-d/%y, %-I:%M %p")
            .to_string(),
        None => "--".into(),
    }
}

pub fn format_date_long(time: Option<SystemTime>) -> String {
    match time {
        Some(t) => DateTime::<Local>::from(t)
            .format("%B %-d, %Y at %-I:%M %p")
            .to_string(),
        None => "--".into(),
    }
}

pub fn format_date_relative(time: Option<SystemTime>) -> String {
    let Some(t) = time else { return "--".into() };
    let dt = DateTime::<Local>::from(t);
    let now = Local::now();
    if dt.date_naive() == now.date_naive() {
        dt.format("Today, %-I:%M %p").to_string()
    } else if dt.date_naive().succ_opt() == Some(now.date_naive()) {
        dt.format("Yesterday, %-I:%M %p").to_string()
    } else if dt.year() == now.year() {
        dt.format("%B %-d at %-I:%M %p").to_string()
    } else {
        format_date_long(time)
    }
}

// ---------------------------------------------------------------------------
// Recursive folder size
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub enum FolderSize {
    Computing,
    Done { bytes: u64, complete: bool },
}

pub fn folder_size(root: &Path, max_entries: usize) -> (u64, bool) {
    let mut total = 0u64;
    let mut seen = 0usize;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for item in read.flatten() {
            seen += 1;
            if seen > max_entries {
                return (total, false);
            }
            let Ok(meta) = item.metadata() else { continue };
            if meta.is_dir() {
                stack.push(item.path());
            } else {
                total += meta.len();
            }
        }
    }
    (total, true)
}

// ---------------------------------------------------------------------------
// Recursive name search
// ---------------------------------------------------------------------------

pub fn search(
    root: &Path,
    query: &str,
    show_hidden: bool,
    max_results: usize,
    max_visited: usize,
) -> Vec<Entry> {
    let needle = query.to_lowercase();
    let mut results = Vec::new();
    let mut visited = 0usize;
    let mut queue = std::collections::VecDeque::from([root.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for item in read.flatten() {
            visited += 1;
            if visited > max_visited || results.len() >= max_results {
                return results;
            }
            let name = item.file_name().to_string_lossy().into_owned();
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            let path = item.path();
            let is_real_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let is_app = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("app"));
            if name.to_lowercase().contains(&needle)
                && let Some(entry) = Entry::with_name(&path, name)
            {
                results.push(entry);
            }
            if is_real_dir && !is_app {
                queue.push_back(path);
            }
        }
    }
    results
}
