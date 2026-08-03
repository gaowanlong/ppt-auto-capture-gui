use std::collections::VecDeque;

use chrono::{DateTime, Local};

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

#[cfg(test)]
mod tests {
    use super::*;

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
