use std::process::ExitCode;

const USAGE: &str = "Usage:\n  mailmap [--domain <domain>] <repository>...\n  mailmap -h | --help\n  mailmap --version\n\nOptions:\n  --domain <domain>     Prefer identities with email addresses on this domain.\n  -h --help             Show this screen.\n  --version             Show version.";

fn main() -> ExitCode {
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("mailmap requires `git` to be installed and accessible on path");
        return ExitCode::from(1);
    }

    let mut repositories = Vec::new();
    let mut preferred_domain = None;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("-h" | "--help") => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            Some("--version") => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            Some("--domain") => {
                let Some(domain) = arguments.next() else {
                    eprintln!("--domain requires a domain value\n\n{USAGE}");
                    return ExitCode::from(2);
                };
                preferred_domain = Some(domain.to_string_lossy().into_owned());
            }
            Some(value) if value.starts_with("--domain=") => {
                let domain = &value["--domain=".len()..];
                if domain.is_empty() {
                    eprintln!("--domain requires a domain value\n\n{USAGE}");
                    return ExitCode::from(2);
                }
                preferred_domain = Some(domain.to_owned());
            }
            Some(value) if value.starts_with('-') => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            _ => repositories.push(argument),
        }
    }

    if repositories.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }

    let preferred_domain = preferred_domain.as_deref();
    match mailmap_generator::create_mailmap_with_domain(&repositories, preferred_domain) {
        Ok(mailmap) => {
            print!("{mailmap}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("mailmap: {error}");
            ExitCode::from(1)
        }
    }
}
