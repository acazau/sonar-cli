use std::path::PathBuf;

#[derive(clap::Args)]
pub struct SetupReportsArgs {
    /// Base directory for reports
    #[arg(long, default_value = "./reports")]
    pub base_dir: String,
    /// Output JSON instead of plain path
    #[arg(long)]
    pub json: bool,
}

pub fn setup_reports(args: &SetupReportsArgs) {
    let now = chrono_stamp();
    let base = PathBuf::from(&args.base_dir);
    let report_root = base.join(&now);
    std::fs::create_dir_all(&report_root).expect("failed to create report root");

    let abs_root = std::fs::canonicalize(&report_root).expect("failed to resolve absolute path");

    if args.json {
        println!("{{\"report_root\":\"{}\"}}", abs_root.display());
    } else {
        println!("{}", abs_root.display());
    }
}

pub fn chrono_stamp() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock before epoch");
    let secs = now.as_secs();

    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let (year, month, day) = days_to_ymd(days);
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", year, month, day, hours, minutes, seconds)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = days / 146097;
    let doe = days % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn setup_reports_creates_structure() {
        let tmp = env::temp_dir().join(format!("xtask-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let args = SetupReportsArgs { base_dir: tmp.to_str().unwrap().to_string(), json: false };
        setup_reports(&args);
        let entries: Vec<_> = fs::read_dir(&tmp).unwrap().collect();
        assert_eq!(entries.len(), 1, "expected one timestamp directory");
        let ts_dir = entries[0].as_ref().unwrap().path();
        assert!(!ts_dir.join("clippy").exists());
        assert!(!ts_dir.join("tests").exists());
        assert!(ts_dir.is_absolute() || fs::canonicalize(&ts_dir).unwrap().is_absolute());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn setup_reports_json_output() {
        let tmp = env::temp_dir().join(format!("xtask-json-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let args = SetupReportsArgs { base_dir: tmp.to_str().unwrap().to_string(), json: true };
        setup_reports(&args);
        let entries: Vec<_> = fs::read_dir(&tmp).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let ts_dir = entries[0].as_ref().unwrap().path();
        assert!(!ts_dir.join("clippy").exists());
        assert!(!ts_dir.join("tests").exists());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn setup_reports_base_dir_override() {
        let tmp = env::temp_dir().join(format!("xtask-base-{}", std::process::id()));
        let custom = tmp.join("custom-reports");
        let _ = fs::remove_dir_all(&tmp);
        let args = SetupReportsArgs { base_dir: custom.to_str().unwrap().to_string(), json: false };
        setup_reports(&args);
        assert!(custom.is_dir());
        let entries: Vec<_> = fs::read_dir(&custom).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn chrono_stamp_format() {
        let stamp = chrono_stamp();
        assert_eq!(stamp.len(), 15, "stamp should be 15 chars: {}", stamp);
        assert_eq!(&stamp[8..9], "-", "separator should be dash: {}", stamp);
        for (i, c) in stamp.chars().enumerate() {
            if i == 8 { continue; }
            assert!(c.is_ascii_digit(), "char {} should be digit: {}", i, stamp);
        }
    }
}
