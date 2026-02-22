mod client;
mod commands;
mod helpers;
mod output;
mod types;

use clap::{Parser, Subcommand};

use client::SonarQubeConfig;

#[derive(Parser)]
#[command(
    name = "sonar-cli",
    version,
    about = "Standalone CLI for SonarQube — query issues, metrics, rules, and more",
    long_about = "Standalone CLI for SonarQube — query issues, metrics, rules, and more.\n\n\
        Discover projects:   sonar-cli projects\n\
        Inspect a project:   sonar-cli --project KEY issues\n\
        Check quality gate:  sonar-cli --project KEY quality-gate\n\
        Browse metrics:      sonar-cli --project KEY measures\n\
        View metric trends:  sonar-cli --project KEY history --metrics coverage\n\
        Search rules:        sonar-cli rules --language java\n\
        Read source code:    sonar-cli source PROJECT:path/to/file.rs\n\n\
        Most commands require --project (or SONAR_PROJECT_KEY env var).\n\
        Use 'sonar-cli <command> --help' for detailed usage of each command."
)]
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
    /// Check SonarQube server health (no auth required)
    #[command(long_about = "Check SonarQube server health (no auth required).\n\n\
        Returns the server status (UP, STARTING, DOWN). Does not require\n\
        --project or --token.\n\n\
        Examples:\n  \
          sonar-cli health\n  \
          sonar-cli --url https://sonar.example.com health")]
    Health,

    /// Check quality gate status (requires --project)
    #[command(name = "quality-gate", long_about = "Check quality gate status (requires --project).\n\n\
        Shows whether the project passes its quality gate and lists each\n\
        condition with its actual value vs threshold.\n\n\
        Examples:\n  \
          sonar-cli --project my-proj quality-gate\n  \
          sonar-cli --project my-proj quality-gate --fail-on-error")]
    QualityGate {
        /// Exit with code 1 if quality gate fails (useful in CI)
        #[arg(long)]
        fail_on_error: bool,
    },

    /// Search and filter project issues (requires --project)
    #[command(long_about = "Search and filter project issues (requires --project).\n\n\
        By default only shows open issues (OPEN, CONFIRMED, REOPENED).\n\
        Use --status to query other statuses like RESOLVED or CLOSED.\n\
        Use 'rules' command to discover rule keys for --rule filter.\n\n\
        Examples:\n  \
          sonar-cli --project my-proj issues\n  \
          sonar-cli --project my-proj issues --severity CRITICAL\n  \
          sonar-cli --project my-proj issues --status RESOLVED --language java\n  \
          sonar-cli --project my-proj issues --created-after 2025-01-01 --limit 50")]
    Issues {
        /// Minimum severity — shows this level and above (INFO, MINOR, MAJOR, CRITICAL, BLOCKER)
        #[arg(long)]
        severity: Option<String>,

        /// Issue type filter (BUG, VULNERABILITY, CODE_SMELL, SECURITY_HOTSPOT)
        #[arg(long, name = "type")]
        issue_type: Option<String>,

        /// Maximum number of issues to return
        #[arg(long)]
        limit: Option<usize>,

        /// Status filter [default: OPEN,CONFIRMED,REOPENED] (OPEN, CONFIRMED, REOPENED, RESOLVED, CLOSED)
        #[arg(long)]
        status: Option<String>,

        /// Resolution filter — only applies to RESOLVED/CLOSED issues (FALSE-POSITIVE, WONTFIX, FIXED, REMOVED)
        #[arg(long)]
        resolution: Option<String>,

        /// Tags filter (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Rule key filter (comma-separated, e.g. java:S1234 — use 'rules' command to discover keys)
        #[arg(long)]
        rule: Option<String>,

        /// Only issues created after this date (YYYY-MM-DD)
        #[arg(long)]
        created_after: Option<String>,

        /// Only issues created before this date (YYYY-MM-DD)
        #[arg(long)]
        created_before: Option<String>,

        /// Filter by issue author login
        #[arg(long)]
        author: Option<String>,

        /// Assignee filter (comma-separated logins, use 'unassigned' for unassigned issues)
        #[arg(long)]
        assignee: Option<String>,

        /// Language filter (comma-separated, e.g. java,py,js)
        #[arg(long)]
        language: Option<String>,
    },

    /// Get project metrics (requires --project)
    #[command(long_about = "Get project metrics (requires --project).\n\n\
        Shows current metric values for the project. If --metrics is omitted,\n\
        returns a default set of common metrics.\n\n\
        Common metric keys: ncloc, coverage, bugs, vulnerabilities, code_smells,\n\
        duplicated_lines_density, sqale_index, reliability_rating, security_rating.\n\
        Use 'history' command to view how these metrics change over time.\n\n\
        Examples:\n  \
          sonar-cli --project my-proj measures\n  \
          sonar-cli --project my-proj measures --metrics coverage,bugs,ncloc")]
    Measures {
        /// Comma-separated metric keys (common keys: ncloc, coverage, bugs, vulnerabilities, code_smells)
        #[arg(long)]
        metrics: Option<String>,
    },

    /// Per-file coverage breakdown (requires --project)
    #[command(long_about = "Per-file coverage breakdown (requires --project).\n\n\
        Lists every file in the project with its coverage percentage,\n\
        uncovered lines, and total coverable lines.\n\n\
        Examples:\n  \
          sonar-cli --project my-proj coverage\n  \
          sonar-cli --project my-proj coverage --min-coverage 80\n  \
          sonar-cli --project my-proj coverage --sort uncovered")]
    Coverage {
        /// Only show files below this coverage percentage (e.g. 80)
        #[arg(long)]
        min_coverage: Option<f64>,

        /// Sort by: coverage (default), uncovered, file
        #[arg(long)]
        sort: Option<String>,
    },

    /// Code duplication analysis (requires --project)
    #[command(long_about = "Code duplication analysis (requires --project).\n\n\
        Lists files with duplicated code, showing duplicated lines and density.\n\
        Use --details to see the exact duplicated blocks and where they appear.\n\n\
        Examples:\n  \
          sonar-cli --project my-proj duplications\n  \
          sonar-cli --project my-proj duplications --details")]
    Duplications {
        /// Show detailed duplication blocks (which lines, duplicated where)
        #[arg(long)]
        details: bool,
    },

    /// Security hotspots review (requires --project)
    #[command(long_about = "Security hotspots review (requires --project).\n\n\
        Lists security hotspots that need manual review. By default shows\n\
        only TO_REVIEW hotspots.\n\n\
        Examples:\n  \
          sonar-cli --project my-proj hotspots\n  \
          sonar-cli --project my-proj hotspots --status REVIEWED")]
    Hotspots {
        /// Status filter [default: TO_REVIEW] (TO_REVIEW, REVIEWED)
        #[arg(long)]
        status: Option<String>,
    },

    /// List and search projects on the server (no --project required)
    #[command(long_about = "List and search projects on the server (no --project required).\n\n\
        Discover available projects, their keys, and last analysis date.\n\
        Use the project key from the output with --project in other commands.\n\n\
        Examples:\n  \
          sonar-cli projects\n  \
          sonar-cli projects --search my-app\n  \
          sonar-cli projects --qualifier VW   # list portfolios")]
    Projects {
        /// Search query to filter projects by name or key
        #[arg(long)]
        search: Option<String>,

        /// Component qualifier (TRK=projects, VW=portfolios, APP=applications)
        #[arg(long, default_value = "TRK")]
        qualifier: String,
    },

    /// View metric trends over time (requires --project)
    #[command(long_about = "View metric trends over time (requires --project).\n\n\
        Shows historical values for one or more metrics across analysis runs.\n\
        Use 'measures' command to discover available metric keys for your project.\n\n\
        Common metric keys: coverage, bugs, vulnerabilities, code_smells, ncloc,\n\
        duplicated_lines_density, sqale_index, reliability_rating, security_rating.\n\n\
        Examples:\n  \
          sonar-cli --project my-proj history --metrics coverage\n  \
          sonar-cli --project my-proj history --metrics coverage,bugs --from 2025-01-01\n  \
          sonar-cli --project my-proj history --metrics ncloc --from 2025-01-01 --to 2025-06-01")]
    History {
        /// Comma-separated metric keys (use 'measures' command to discover available keys)
        #[arg(long)]
        metrics: String,

        /// Start date, inclusive (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// End date, inclusive (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,
    },

    /// Search and browse quality rules (no --project required)
    #[command(long_about = "Search and browse quality rules (no --project required).\n\n\
        Discover available rules, their keys, severity, and language.\n\
        Rule keys from this output can be used with 'issues --rule' to filter issues.\n\n\
        Examples:\n  \
          sonar-cli rules\n  \
          sonar-cli rules --language java --severity CRITICAL\n  \
          sonar-cli rules --search \"null pointer\"\n  \
          sonar-cli rules --rule-type BUG --status READY")]
    Rules {
        /// Search query to filter rules by name or description
        #[arg(long)]
        search: Option<String>,

        /// Language filter (e.g. java, py, js, ts, go, cs)
        #[arg(long)]
        language: Option<String>,

        /// Severity filter (INFO, MINOR, MAJOR, CRITICAL, BLOCKER)
        #[arg(long)]
        severity: Option<String>,

        /// Rule type filter (CODE_SMELL, BUG, VULNERABILITY, SECURITY_HOTSPOT)
        #[arg(long, name = "rule-type")]
        rule_type: Option<String>,

        /// Status filter [default: all] (READY, DEPRECATED, BETA, REMOVED)
        #[arg(long)]
        status: Option<String>,
    },

    /// View source code of a file on the server (no --project required)
    #[command(long_about = "View source code of a file on the server (no --project required).\n\n\
        Retrieves the source code as stored in SonarQube. The component key is\n\
        typically PROJECT_KEY:path/to/file. Use 'projects' command to find\n\
        project keys, then browse files.\n\n\
        Without --from/--to, fetches the entire file. With line range options,\n\
        fetches only the specified lines.\n\n\
        Examples:\n  \
          sonar-cli source my-project:src/main.rs\n  \
          sonar-cli source my-project:src/main.rs --from 1 --to 50\n  \
          sonar-cli source my-project:src/main.rs --json")]
    Source {
        /// Component key (format: PROJECT_KEY:path/to/file, e.g. my-project:src/main.rs)
        component: String,

        /// Start line number (fetches full file if omitted)
        #[arg(long)]
        from: Option<usize>,

        /// End line number
        #[arg(long)]
        to: Option<usize>,
    },

    /// Wait for a background analysis task to complete
    #[command(long_about = "Wait for a background analysis task to complete.\n\n\
        After running 'scan', SonarQube processes the report asynchronously.\n\
        Use this command with the task ID to block until the analysis finishes.\n\
        The 'scan --wait' flag does this automatically.\n\n\
        Examples:\n  \
          sonar-cli wait AXyz123abc\n  \
          sonar-cli wait AXyz123abc --timeout 600 --poll-interval 10")]
    Wait {
        /// Analysis task ID (printed by 'scan' command)
        task_id: String,

        /// Maximum wait time in seconds
        #[arg(long, default_value = "300")]
        timeout: u64,

        /// Polling interval in seconds
        #[arg(long, default_value = "5")]
        poll_interval: u64,
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

/// Initialise the tracing subscriber.
///
/// When `verbose` is true, the default log level is `debug`; otherwise `warn`.
/// Both cases respect the `RUST_LOG` environment variable.
fn init_tracing(verbose: bool) {
    let default_filter = if verbose { "sonar_cli=debug" } else { "sonar_cli=warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| default_filter.into()),
        )
        .with_target(false)
        .init();
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();

    init_tracing(cli.verbose);

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
            ref status,
            ref resolution,
            ref tags,
            ref rule,
            ref created_after,
            ref created_before,
            ref author,
            ref assignee,
            ref language,
        } => {
            let project = match cli.require_project() {
                Ok(p) => p,
                Err(code) => std::process::exit(code),
            };
            let params = commands::issues::IssuesCommandParams {
                min_severity: severity.as_deref(),
                issue_type: issue_type.as_deref(),
                limit,
                statuses: status.as_deref(),
                resolutions: resolution.as_deref(),
                tags: tags.as_deref(),
                rules: rule.as_deref(),
                created_after: created_after.as_deref(),
                created_before: created_before.as_deref(),
                author: author.as_deref(),
                assignees: assignee.as_deref(),
                languages: language.as_deref(),
            };
            commands::issues::run(config, project, &params, cli.json).await
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

        Command::Projects {
            ref search,
            ref qualifier,
        } => {
            commands::projects::run(config, search.as_deref(), Some(qualifier.as_str()), cli.json)
                .await
        }

        Command::History {
            ref metrics,
            ref from,
            ref to,
        } => {
            let project = match cli.require_project() {
                Ok(p) => p,
                Err(code) => std::process::exit(code),
            };
            commands::history::run(
                config,
                project,
                metrics,
                from.as_deref(),
                to.as_deref(),
                cli.json,
            )
            .await
        }

        Command::Rules {
            ref search,
            ref language,
            ref severity,
            ref rule_type,
            ref status,
        } => {
            commands::rules::run(
                config,
                search.as_deref(),
                language.as_deref(),
                severity.as_deref(),
                rule_type.as_deref(),
                status.as_deref(),
                cli.json,
            )
            .await
        }

        Command::Source {
            ref component,
            from,
            to,
        } => commands::source::run(config, component, from, to, cli.json).await,

        Command::Wait {
            task_id,
            timeout,
            poll_interval,
        } => commands::wait::run(config, &task_id, timeout, poll_interval, cli.json).await,

    };

    std::process::exit(exit_code);
}
