//! `software-design`: design red flags in TypeScript, JavaScript, React, Vue, Python, PHP and Rust.

mod analysis;
mod hook;
mod mcp;

use std::io::Read;
use std::process::ExitCode;

use history::Repo;
use rules::Tier;

use crate::analysis::{DEFAULT_SINCE, Lines, Request};

const USAGE: &str = "\
software-design: design red flags in TypeScript, JavaScript, React, Vue, Python, PHP and Rust

Usage:
  software-design check [--tier A|B] [--diff | --base <rev>] [--since <date>] [--strict] [PATH...]
  software-design interface PATH...
  software-design hook <session-start|post-edit|stop|user-prompt>
  software-design mcp [--require-directory]

Commands:
  check      Print findings as JSON. Tier A (default) is precise and file-local;
             tier B adds candidates that need review and reads the whole repository.
  interface  Print public signatures and interface comments, without bodies.
  hook       Handle a Claude Code hook event: JSON on stdin, JSON on stdout.
  mcp        Serve `check` and `interface` as MCP tools over stdio. With
             --require-directory, each call must name the repository's directory.

Options:
  --diff          Report only lines changed since HEAD, including uncommitted work.
  --base <rev>    Report only lines changed since <rev>.
  --since <date>  History window for churn and co-change (default: 12 months ago).
  --strict        Exit with status 1 when there are findings.
  -h, --help      Print this help.
  -V, --version   Print the version.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check") => check(&args[1..]),
        Some("interface") => interface(&args[1..]),
        Some("hook") => run_hook(&args[1..]),
        Some("mcp") => mcp::serve(args[1..] == ["--require-directory"])
            .map(|()| ExitCode::SUCCESS)
            .map_err(Failure::from),
        Some("-V" | "--version") => {
            println!("software-design {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        Some("-h" | "--help") => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(Usage("missing or unknown command".to_owned()).into()),
    };
    result.unwrap_or_else(|failure| match failure {
        Failure::Usage(message) => {
            eprintln!("software-design: {message}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Failure::Runtime(message) => {
            eprintln!("software-design: {message}");
            ExitCode::FAILURE
        }
    })
}

enum Failure {
    /// Bad arguments: exit status 2.
    Usage(String),
    /// The work failed: exit status 1.
    Runtime(String),
}

struct Usage(String);

impl From<Usage> for Failure {
    fn from(usage: Usage) -> Failure {
        Failure::Usage(usage.0)
    }
}

impl From<String> for Failure {
    fn from(message: String) -> Failure {
        Failure::Runtime(message)
    }
}

fn check(args: &[String]) -> Result<ExitCode, Failure> {
    let mut request = Request {
        paths: Vec::new(),
        lines: Lines::All,
        tier: Tier::A,
        since: DEFAULT_SINCE.to_owned(),
    };
    let mut strict = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .cloned()
                .ok_or_else(|| Usage(format!("{flag} needs a value")))
        };
        match arg.as_str() {
            "--tier" => {
                request.tier = value("--tier")?.parse().map_err(Usage)?;
            }
            "--diff" => request.lines = Lines::AddedSince(None),
            "--base" => request.lines = Lines::AddedSince(Some(value("--base")?)),
            "--since" => request.since = value("--since")?,
            "--strict" => strict = true,
            flag if flag.starts_with('-') => {
                return Err(Usage(format!("unknown option {flag}")).into());
            }
            path => request.paths.push(path.to_owned()),
        }
    }
    let (repo, cwd) = repo()?;
    request.paths = request
        .paths
        .iter()
        .map(|p| analysis::relative(&repo, &cwd, p))
        .collect();
    let findings = analysis::check(&repo, &request)?;
    println!("{:#}", analysis::report(&repo, &findings));
    Ok(if strict && !findings.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn interface(args: &[String]) -> Result<ExitCode, Failure> {
    if args.is_empty() || args.iter().any(|a| a.starts_with('-')) {
        return Err(Usage("interface takes one or more paths".to_owned()).into());
    }
    let (repo, cwd) = repo()?;
    let paths: Vec<String> = args
        .iter()
        .map(|p| analysis::relative(&repo, &cwd, p))
        .collect();
    print!("{}", analysis::interface(&repo, &paths)?);
    Ok(ExitCode::SUCCESS)
}

/// Hooks always exit 0: a broken analysis must not break the user's session.
fn run_hook(args: &[String]) -> Result<ExitCode, Failure> {
    let [event] = args else {
        return Err(Usage("hook takes one event name".to_owned()).into());
    };
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return Ok(ExitCode::SUCCESS);
    }
    match hook::run(event, &input) {
        Ok(Some(output)) => println!("{output}"),
        Ok(None) => {}
        Err(message) => eprintln!("software-design hook {event}: {message}"),
    }
    Ok(ExitCode::SUCCESS)
}

fn repo() -> Result<(Repo, std::path::PathBuf), Failure> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let repo = Repo::discover(&cwd).map_err(|e| format!("not inside a git repository: {e}"))?;
    Ok((repo, cwd))
}
