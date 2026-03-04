mod clippy;
mod coverage;
mod git;
mod scan;
mod setup;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Dev workflow tasks for sonar-cli")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a timestamped report directory
    SetupReports(setup::SetupReportsArgs),
    /// List in-scope .rs files (changed or all)
    Scope(git::ScopeArgs),
    /// Run clippy and write a JSON report for SonarQube
    ClippyReport(ReportRootArgs),
    /// Run tests with coverage and write a Cobertura XML report for SonarQube
    TestReport(ReportRootArgs),
    /// Run SonarQube scan with auto-detected reports and project defaults
    SonarScan(scan::SonarScanArgs),
    /// Run SonarQube scan inside a Docker container (sonarsource/sonar-scanner-cli)
    DockerScan(scan::DockerScanArgs),
}

#[derive(clap::Args)]
pub struct ReportRootArgs {
    /// Root report directory (subcommand creates its own subdirectory)
    #[arg(long)]
    pub report_root: String,
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Cmd::SetupReports(args) => setup::setup_reports(args),
        Cmd::Scope(args) => git::scope(args),
        Cmd::ClippyReport(args) => clippy::clippy_report(args),
        Cmd::TestReport(args) => coverage::test_report(args),
        Cmd::SonarScan(args) => scan::sonar_scan(args),
        Cmd::DockerScan(args) => scan::docker_scan(args),
    }
}
