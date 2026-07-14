use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;

use fileos_app::{CancellationToken, ScanPort, ScanProgress, ScanRequest, ScanSink, ScanSinkError};
use fileos_domain::{ObservedEntry, ResourceLimits, ScanIssue, ScanRunStatus};
use fileos_scanner::ReadOnlyScanner;

const EXIT_SUCCESS: u8 = 0;
const EXIT_FAILURE: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_PARTIAL: u8 = 3;
const EXIT_CANCELLED: u8 = 4;
const EXIT_IO_ERROR: u8 = 74;
const USAGE: &str = "Usage: fileos-cli <absolute-scan-root>\n";

fn main() -> ExitCode {
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut output = stdout.lock();
    let mut errors = stderr.lock();
    ExitCode::from(execute(env::args_os().skip(1), &mut output, &mut errors))
}

fn execute(
    mut arguments: impl Iterator<Item = OsString>,
    output: &mut dyn Write,
    errors: &mut dyn Write,
) -> u8 {
    let Some(root_argument) = arguments.next() else {
        return write_usage_error(errors);
    };
    if root_argument == OsStr::new("--help") || root_argument == OsStr::new("-h") {
        if arguments.next().is_some() {
            return write_usage_error(errors);
        }
        return if output.write_all(USAGE.as_bytes()).is_ok() {
            EXIT_SUCCESS
        } else {
            EXIT_IO_ERROR
        };
    }
    if arguments.next().is_some() {
        return write_usage_error(errors);
    }

    let limits = match default_limits() {
        Some(limits) => limits,
        None => return write_safe_error(errors, "Internal CLI limits are invalid.\n"),
    };
    let request = match ScanRequest::new(PathBuf::from(root_argument), limits) {
        Ok(request) => request,
        Err(error) => return write_safe_error(errors, &format!("{error}\n")),
    };
    let scanner = ReadOnlyScanner::new();
    let mut sink = DiscardingSink;
    let report = match scanner.scan(&request, &CancellationToken::new(), &mut sink) {
        Ok(report) => report,
        Err(error) => return write_safe_error(errors, &format!("{error}\n")),
    };
    if output.write_all(report.to_json().as_bytes()).is_err() {
        return EXIT_IO_ERROR;
    }
    match report.status() {
        ScanRunStatus::Completed => EXIT_SUCCESS,
        ScanRunStatus::CompletedWithIssues => EXIT_PARTIAL,
        ScanRunStatus::Cancelled => EXIT_CANCELLED,
        _ => EXIT_FAILURE,
    }
}

fn default_limits() -> Option<ResourceLimits> {
    Some(ResourceLimits::new(
        NonZeroUsize::new(1)?,
        NonZeroUsize::new(64)?,
        NonZeroUsize::new(64)?,
        NonZeroUsize::new(256)?,
        NonZeroUsize::new(128)?,
    ))
}

fn write_usage_error(errors: &mut dyn Write) -> u8 {
    if errors.write_all(USAGE.as_bytes()).is_ok() {
        EXIT_USAGE
    } else {
        EXIT_IO_ERROR
    }
}

fn write_safe_error(errors: &mut dyn Write, message: &str) -> u8 {
    if errors.write_all(message.as_bytes()).is_ok() {
        EXIT_FAILURE
    } else {
        EXIT_IO_ERROR
    }
}

struct DiscardingSink;

impl ScanSink for DiscardingSink {
    fn observe(&mut self, _entry: ObservedEntry) -> Result<(), ScanSinkError> {
        Ok(())
    }

    fn issue(&mut self, _issue: ScanIssue) -> Result<(), ScanSinkError> {
        Ok(())
    }

    fn progress(&mut self, _progress: ScanProgress) -> Result<(), ScanSinkError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use fileos_testkit::SandboxBase;

    use super::{EXIT_SUCCESS, EXIT_USAGE, execute};

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("CLI crate must remain below the repository root")
            .to_path_buf()
    }

    #[test]
    fn fixture_scan_emits_path_free_json_and_preserves_source() {
        let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
        let run = base.create_fixture(10_010).expect("fixture generation");
        let before = run.snapshot().expect("before CLI snapshot");
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let code = execute(
            [run.scan_root().as_os_str().to_owned()].into_iter(),
            &mut output,
            &mut errors,
        );

        assert_eq!(code, EXIT_SUCCESS);
        assert!(errors.is_empty());
        let json = String::from_utf8(output).expect("UTF-8 summary JSON");
        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"status\":\"completed\""));
        assert!(!json.contains(&run.path().to_string_lossy().into_owned()));
        assert!(!json.contains("sample.bin"));
        println!("MH_FILEOS_SCAN_JSON_BEGIN");
        print!("{json}");
        println!("MH_FILEOS_SCAN_JSON_END");
        assert_eq!(before, run.snapshot().expect("after CLI snapshot"));
        run.cleanup().expect("safe fixture cleanup");
    }

    #[test]
    fn help_and_invalid_root_do_not_touch_filesystem() {
        let mut output = Vec::new();
        let mut errors = Vec::new();
        assert_eq!(
            execute(
                [OsString::from("--help")].into_iter(),
                &mut output,
                &mut errors,
            ),
            EXIT_SUCCESS
        );
        assert!(errors.is_empty());

        output.clear();
        assert_eq!(
            execute(
                [OsString::from("relative-root")].into_iter(),
                &mut output,
                &mut errors,
            ),
            super::EXIT_FAILURE
        );
        let safe_error = String::from_utf8(errors).expect("UTF-8 safe error");
        assert!(!safe_error.contains("relative-root"));

        let mut no_args_output = Vec::new();
        let mut no_args_errors = Vec::new();
        assert_eq!(
            execute(std::iter::empty(), &mut no_args_output, &mut no_args_errors,),
            EXIT_USAGE
        );
    }
}
