mod client;
mod commands;
mod coverage;
mod output;
mod scanner;
mod types;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use client::SonarQubeConfig;

#[derive(Parser)]
#[command(name = "sonar-cli", version, about = "Standalone CLI for SonarQube")]
struct Cli {
    /// SonarQube server URL
    #[arg(long, env = "SONAR_HOST_URL", default_value = "http://localhost:9000", global = true)]
    url: String,

    /// Authentication token
    #[arg(long, env = "SONAR_TOKEN", global = true)]
    token: Option<String>,

    /// Project key
    #[arg(long, env = "SONAR_PROJECT_KEY", global = true)]
    project: Option<String>,

    /// Branch name
    #[arg(long, env = "SONAR_BRANCH", global = true)]
    branch: Option<String>,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Request timeout in seconds
    #[arg(long, default_value = "30", global = true)]
    timeout: u64,

    /// Verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check SonarQube server health
    Health,

    /// Check quality gate status
    #[command(name = "quality-gate")]
    QualityGate {
        /// Exit with code 1 if quality gate fails
        #[arg(long)]
        fail_on_error: bool,
    },

    /// List project issues
    Issues {
        /// Minimum severity filter (INFO, MINOR, MAJOR, CRITICAL, BLOCKER)
        #[arg(long)]
        severity: Option<String>,

        /// Issue type filter (BUG, VULNERABILITY, CODE_SMELL, SECURITY_HOTSPOT)
        #[arg(long, name = "type")]
        issue_type: Option<String>,

        /// Maximum number of issues to show
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Get project metrics
    Measures {
        /// Comma-separated metric keys
        #[arg(long)]
        metrics: Option<String>,
    },

    /// Per-file coverage breakdown
    Coverage {
        /// Only show files below this coverage percentage
        #[arg(long)]
        min_coverage: Option<f64>,

        /// Sort by: coverage (default), uncovered, file
        #[arg(long)]
        sort: Option<String>,
    },

    /// Code duplication information
    Duplications {
        /// Show detailed duplication blocks
        #[arg(long)]
        details: bool,
    },

    /// Security hotspots
    Hotspots {
        /// Status filter (default: TO_REVIEW)
        #[arg(long)]
        status: Option<String>,
    },

    /// Wait for analysis task completion
    Wait {
        /// Task ID
        task_id: String,

        /// Maximum wait time in seconds
        #[arg(long, default_value = "300")]
        timeout: u64,

        /// Polling interval in seconds
        #[arg(long, default_value = "5")]
        poll_interval: u64,
    },

    /// Run sonar-scanner analysis
    Scan {
        /// Project root directory
        #[arg(long, default_value = ".")]
        source_dir: PathBuf,

        /// Source directories (comma-separated)
        #[arg(long, default_value = "src")]
        sources: String,

        /// Test directories (comma-separated)
        #[arg(long)]
        tests: Option<String>,

        /// Exclusion globs (comma-separated)
        #[arg(long)]
        exclusions: Option<String>,

        /// Coverage report path (auto-detects Cobertura format)
        #[arg(long)]
        coverage_report: Option<String>,

        /// Path to sonar-scanner binary
        #[arg(long, default_value = "sonar-scanner")]
        scanner_path: String,

        /// Use Docker to run scanner
        #[arg(long)]
        docker: bool,

        /// Custom Docker image
        #[arg(long)]
        docker_image: Option<String>,

        /// Wait for analysis completion and show results
        #[arg(long)]
        wait: bool,

        /// Extra SonarQube properties (key=value, repeatable)
        #[arg(short = 'D', number_of_values = 1)]
        props: Vec<String>,
    },
}

impl Cli {
    fn build_config(&self) -> SonarQubeConfig {
        let mut config = SonarQubeConfig::new(&self.url)
            .with_timeout(std::time::Duration::from_secs(self.timeout));

        if let Some(ref token) = self.token {
            config = config.with_token(token);
        }
        if let Some(ref project) = self.project {
            config = config.with_project(project);
        }
        if let Some(ref branch) = self.branch {
            config = config.with_branch(branch);
        }
        config
    }

    fn require_project(&self) -> Result<&str, i32> {
        self.project.as_deref().ok_or_else(|| {
            eprintln!("Project key is required. Use --project or set SONAR_PROJECT_KEY.");
            1
        })
    }
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "sonar_cli=debug".into()),
            )
            .with_target(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "sonar_cli=warn".into()),
            )
            .with_target(false)
            .init();
    }

    let config = cli.build_config();

    let exit_code = match cli.command {
        Command::Health => commands::health::run(config, cli.json).await,

        Command::QualityGate { fail_on_error } => {
            let project = match cli.require_project() {
                Ok(p) => p,
                Err(code) => std::process::exit(code),
            };
            commands::quality_gate::run(config, project, fail_on_error, cli.json).await
        }

        Command::Issues {
            ref severity,
            ref issue_type,
            limit,
        } => {
            let project = match cli.require_project() {
                Ok(p) => p,
                Err(code) => std::process::exit(code),
            };
            commands::issues::run(
                config,
                project,
                severity.as_deref(),
                issue_type.as_deref(),
                limit,
                cli.json,
            )
            .await
        }

        Command::Measures { ref metrics } => {
            let project = match cli.require_project() {
                Ok(p) => p,
                Err(code) => std::process::exit(code),
            };
            commands::measures::run(config, project, metrics.as_deref(), cli.json).await
        }

        Command::Coverage {
            min_coverage,
            ref sort,
        } => {
            let project = match cli.require_project() {
                Ok(p) => p,
                Err(code) => std::process::exit(code),
            };
            commands::coverage::run(config, project, min_coverage, sort.as_deref(), cli.json).await
        }

        Command::Duplications { details } => {
            let project = match cli.require_project() {
                Ok(p) => p,
                Err(code) => std::process::exit(code),
            };
            commands::duplications::run(config, project, details, cli.json).await
        }

        Command::Hotspots { ref status } => {
            let project = match cli.require_project() {
                Ok(p) => p,
                Err(code) => std::process::exit(code),
            };
            commands::hotspots::run(config, project, status.as_deref(), cli.json).await
        }

        Command::Wait {
            task_id,
            timeout,
            poll_interval,
        } => commands::wait::run(config, &task_id, timeout, poll_interval, cli.json).await,

        Command::Scan {
            source_dir,
            sources,
            tests,
            exclusions,
            coverage_report,
            scanner_path,
            docker,
            docker_image,
            wait,
            props,
        } => {
            commands::scan::run(
                config,
                source_dir,
                sources.split(',').map(|s| s.trim().to_string()).collect(),
                tests
                    .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default(),
                exclusions
                    .map(|e| e.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default(),
                coverage_report,
                scanner_path,
                docker,
                docker_image,
                wait,
                props,
                cli.json,
            )
            .await
        }
    };

    std::process::exit(exit_code);
}
