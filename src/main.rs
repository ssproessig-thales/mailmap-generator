use std::process::ExitCode;

const USAGE: &str = "Usage:\n  mailmap <repository>...\n  mailmap -h | --help\n  mailmap --version\n\nOptions:\n  -h --help             Show this screen.\n  --version             Show version.";

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
    for argument in std::env::args_os().skip(1) {
        match argument.to_str() {
            Some("-h" | "--help") => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            Some("--version") => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
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

    match mailmap_generator::create_mailmap(&repositories) {
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
