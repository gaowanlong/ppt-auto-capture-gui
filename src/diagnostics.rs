use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Local};

static LOG_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for DiagnosticLevel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
        };
        formatter.write_str(label)
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub timestamp: DateTime<Local>,
    pub level: DiagnosticLevel,
    pub category: String,
    pub message: String,
}

impl DiagnosticEntry {
    pub fn new(
        level: DiagnosticLevel,
        category: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: Local::now(),
            level,
            category: category.into(),
            message: message.into(),
        }
    }
}

pub struct DiagnosticHistory {
    capacity: usize,
    entries: VecDeque<DiagnosticEntry>,
}

impl DiagnosticHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.clamp(1, 20),
            entries: VecDeque::new(),
        }
    }

    pub fn push(&mut self, entry: DiagnosticEntry) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn entries_newest_first(&self) -> impl Iterator<Item = &DiagnosticEntry> {
        self.entries.iter().rev()
    }
}

pub struct DiagnosticReportContext<'a> {
    pub version: &'a str,
    pub os: &'a str,
    pub architecture: &'a str,
    pub capture_state: &'a str,
    pub source: &'a str,
    pub display: &'a str,
    pub output_path: &'a str,
    pub log_directory: Option<&'a str>,
}

pub fn render_diagnostic_report(
    context: DiagnosticReportContext<'_>,
    history: &DiagnosticHistory,
) -> String {
    let mut report = format!(
        "PPT Auto Capture Diagnostic Report\n\
         Version: {}\n\
         Platform: {}/{}\n\
         State: {}\n\
         Source: {}\n\
         Display: {}\n\
         Output: {}\n\
         Logs: {}\n\
         Recent diagnostics:\n",
        context.version,
        context.os,
        context.architecture,
        context.capture_state,
        context.source,
        context.display,
        context.output_path,
        context.log_directory.unwrap_or("Unavailable"),
    );

    for entry in history.entries_newest_first() {
        report.push_str(&format!(
            "{} [{}] {}: {}\n",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S %:z"),
            entry.level,
            entry.category,
            entry.message,
        ));
    }

    report
}

pub struct LoggingSetup {
    pub log_directory: Option<PathBuf>,
    pub log_file: Option<PathBuf>,
    pub warning: Option<String>,
}

pub fn resolve_log_directory(home: Option<&Path>, is_macos: bool, current_dir: &Path) -> PathBuf {
    if is_macos {
        if let Some(home) = home {
            return home.join("Library/Logs/PPT Auto Capture");
        }
    }

    current_dir.join("logs")
}

pub fn retain_recent_logs(directory: &Path, keep: usize) -> io::Result<()> {
    let mut logs = Vec::new();

    for entry in fs::read_dir(directory)? {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !name.starts_with("ppt-auto-capture-") || !name.ends_with(".log") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };

        logs.push((modified, name, entry.path()));
    }

    logs.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));

    remove_expired_logs(
        logs.into_iter().skip(keep).map(|(_, _, path)| path),
        |path| fs::remove_file(path),
    )
}

fn remove_expired_logs<F>(
    paths: impl IntoIterator<Item = PathBuf>,
    mut remove_file: F,
) -> io::Result<()>
where
    F: FnMut(&Path) -> io::Result<()>,
{
    let mut failures = Vec::new();

    for path in paths {
        if let Err(error) = remove_file(&path) {
            failures.push(format!("{}: {error}", path.display()));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "Could not remove expired diagnostics logs: {}",
            failures.join("; ")
        )))
    }
}

struct TeeWriter {
    file: Option<BufWriter<File>>,
}

impl TeeWriter {
    fn new(file: Option<File>) -> Self {
        Self {
            file: file.map(BufWriter::new),
        }
    }
}

impl Write for TeeWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let stderr_result = io::stderr().lock().write_all(buffer);

        if let Some(file) = self.file.as_mut() {
            if file.write_all(buffer).and_then(|_| file.flush()).is_err() {
                self.file = None;
            }
        }

        stderr_result.map(|_| buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stderr().lock().flush()?;
        if let Some(file) = self.file.as_mut() {
            if file.flush().is_err() {
                self.file = None;
            }
        }
        Ok(())
    }
}

fn prepare_logging(directory: PathBuf) -> LoggingSetup {
    prepare_logging_with_writer(directory).0
}

fn prepare_logging_with_writer(directory: PathBuf) -> (LoggingSetup, TeeWriter) {
    let result = (|| -> io::Result<(PathBuf, File, Option<String>)> {
        fs::create_dir_all(&directory)?;

        let sequence = LOG_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let file_path = directory.join(format!(
            "ppt-auto-capture-{}-{sequence:020}.log",
            Local::now().format("%Y%m%d-%H%M%S%.3f")
        ));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
        let retention_warning = retain_recent_logs(&directory, 10)
            .err()
            .map(|error| format!("Persistent log retention could not be completed: {error}"));

        Ok((file_path, file, retention_warning))
    })();

    match result {
        Ok((file_path, file, warning)) => (
            LoggingSetup {
                log_directory: Some(directory),
                log_file: Some(file_path),
                warning,
            },
            TeeWriter::new(Some(file)),
        ),
        Err(error) => (
            LoggingSetup {
                log_directory: None,
                log_file: None,
                warning: Some(format!("Persistent logging is unavailable: {error}")),
            },
            TeeWriter::new(None),
        ),
    }
}

pub fn initialize_logging() -> LoggingSetup {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let directory = resolve_log_directory(
        std::env::var_os("HOME").as_deref().map(Path::new),
        cfg!(target_os = "macos"),
        &current_dir,
    );
    let (mut setup, writer) = prepare_logging_with_writer(directory);

    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    builder
        .format_timestamp_millis()
        .target(env_logger::Target::Pipe(Box::new(writer)));
    if let Err(error) = builder.try_init() {
        setup.log_directory = None;
        setup.log_file = None;
        let warning = format!("Logger initialization failed: {error}");
        setup.warning = Some(match setup.warning.take() {
            Some(existing) => format!("{existing}; {warning}"),
            None => warning,
        });
    }

    setup
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs::{self, File, FileTimes};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ppt-auto-capture-diagnostics-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn macos_log_directory_uses_the_users_library_logs_folder() {
        assert_eq!(
            resolve_log_directory(Some(Path::new("/Users/test")), true, Path::new("/work")),
            Path::new("/Users/test/Library/Logs/PPT Auto Capture")
        );
    }

    #[test]
    fn retention_keeps_only_the_ten_newest_matching_regular_files() {
        let directory = TestDirectory::new();
        let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);

        for index in 0..12 {
            let path = directory
                .path
                .join(format!("ppt-auto-capture-20260803-1200{index:02}.log"));
            fs::write(&path, "log").unwrap();
            File::open(&path)
                .unwrap()
                .set_times(FileTimes::new().set_modified(timestamp))
                .unwrap();
        }
        fs::write(directory.path.join("notes.txt"), "keep").unwrap();
        fs::create_dir(directory.path.join("ppt-auto-capture-child.log")).unwrap();

        retain_recent_logs(&directory.path, 10).unwrap();

        let remaining: BTreeSet<_> = fs::read_dir(&directory.path)
            .unwrap()
            .flatten()
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|file_type| file_type.is_file())
                    .and_then(|_| entry.file_name().into_string().ok())
                    .filter(|name| name.starts_with("ppt-auto-capture-") && name.ends_with(".log"))
            })
            .collect();
        let expected: BTreeSet<_> = (2..12)
            .map(|index| format!("ppt-auto-capture-20260803-1200{index:02}.log"))
            .collect();

        assert_eq!(remaining, expected);
        assert!(directory.path.join("notes.txt").is_file());
        assert!(directory.path.join("ppt-auto-capture-child.log").is_dir());
    }

    #[test]
    fn invalid_log_parent_degrades_to_stderr_only_logging() {
        let directory = TestDirectory::new();
        let parent_file = directory.path.join("not-a-directory");
        fs::write(&parent_file, "not a directory").unwrap();

        let setup = prepare_logging(parent_file.join("logs"));

        assert!(setup.warning.is_some());
        assert_eq!(setup.log_file, None);
    }

    #[test]
    fn successive_log_preparations_use_distinct_log_files() {
        let directory = TestDirectory::new();

        let first = prepare_logging(directory.path.clone());
        let second = prepare_logging(directory.path.clone());

        assert_ne!(first.log_file, second.log_file);
    }

    #[test]
    fn expired_log_removal_continues_after_a_single_failure() {
        let expired = vec![
            PathBuf::from("/diagnostics/ppt-auto-capture-oldest.log"),
            PathBuf::from("/diagnostics/ppt-auto-capture-middle.log"),
            PathBuf::from("/diagnostics/ppt-auto-capture-newest.log"),
        ];
        let mut attempted = Vec::new();

        let error = remove_expired_logs(expired.clone(), |path| {
            attempted.push(path.to_path_buf());
            if path == expired[1] {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
            } else {
                Ok(())
            }
        })
        .expect_err("the failed removal is reported after all expired logs are attempted");

        assert_eq!(attempted, expired);
        assert!(error.to_string().contains("ppt-auto-capture-middle.log"));
    }

    #[test]
    fn history_evicts_the_oldest_entry_at_capacity() {
        let mut history = DiagnosticHistory::new(20);
        for index in 0..21 {
            history.push(DiagnosticEntry::new(
                DiagnosticLevel::Error,
                "capture",
                format!("failure-{index}"),
            ));
        }
        let messages: Vec<_> = history
            .entries_newest_first()
            .map(|entry| entry.message.as_str())
            .collect();
        assert_eq!(messages.len(), 20);
        assert_eq!(messages.first(), Some(&"failure-20"));
        assert!(!messages.contains(&"failure-0"));
    }

    #[test]
    fn history_clamps_zero_capacity_to_one() {
        let mut history = DiagnosticHistory::new(0);
        history.push(DiagnosticEntry::new(
            DiagnosticLevel::Info,
            "capture",
            "first",
        ));
        history.push(DiagnosticEntry::new(
            DiagnosticLevel::Info,
            "capture",
            "second",
        ));

        let messages: Vec<_> = history
            .entries_newest_first()
            .map(|entry| entry.message.as_str())
            .collect();
        assert_eq!(messages, ["second"]);
    }

    #[test]
    fn history_clamps_large_capacity_to_twenty() {
        let mut history = DiagnosticHistory::new(21);
        for index in 0..21 {
            history.push(DiagnosticEntry::new(
                DiagnosticLevel::Warning,
                "capture",
                format!("failure-{index}"),
            ));
        }

        let messages: Vec<_> = history
            .entries_newest_first()
            .map(|entry| entry.message.as_str())
            .collect();
        assert_eq!(messages.len(), 20);
        assert_eq!(messages.first(), Some(&"failure-20"));
        assert!(!messages.contains(&"failure-0"));
    }

    #[test]
    fn report_contains_runtime_context_without_private_payloads() {
        let mut history = DiagnosticHistory::new(20);
        history.push(DiagnosticEntry::new(
            DiagnosticLevel::Error,
            "pptx",
            "First frame save failed",
        ));
        let report = render_diagnostic_report(
            DiagnosticReportContext {
                version: "1.1.0",
                os: "macos",
                architecture: "aarch64",
                capture_state: "Error",
                source: "Full Screen",
                display: "Built-in Display",
                output_path: "/tmp/capture.pptx",
                log_directory: Some("/Users/test/Library/Logs/PPT Auto Capture"),
            },
            &history,
        );
        for expected in [
            "Version: 1.1.0",
            "Platform: macos/aarch64",
            "State: Error",
            "Source: Full Screen",
            "Display: Built-in Display",
            "Output: /tmp/capture.pptx",
            "First frame save failed",
        ] {
            assert!(report.contains(expected), "missing {expected}");
        }
        for forbidden in [
            "pixel bytes",
            "clipboard contents",
            "ppt-auto-capture-config.json",
        ] {
            assert!(!report.contains(forbidden));
        }
    }
}
