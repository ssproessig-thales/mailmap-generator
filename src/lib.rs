use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Author {
    name: String,
    email: String,
    original: String,
}

fn parse_author(author: &str) -> Option<Author> {
    let (name, email) = author.split_once(" <")?;
    let email = email.strip_suffix('>')?;
    Some(Author {
        name: name.to_owned(),
        email: email.to_owned(),
        original: author.to_owned(),
    })
}

fn git_log(path: &Path, format: &str) -> io::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("log")
        .arg(format!("--format={format}"))
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git log failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn get_authors_from_git(path: &Path) -> io::Result<Vec<String>> {
    let output = git_log(path, "%an <%ae>")?;
    Ok(output.lines().map(str::to_owned).collect())
}

fn get_coauthors_from_git(path: &Path) -> io::Result<Vec<String>> {
    let messages = git_log(path, "%B")?;
    Ok(parse_coauthors(&messages))
}

fn parse_coauthors(messages: &str) -> Vec<String> {
    let mut coauthors = Vec::new();

    for line in messages.lines() {
        let Some(rest) = line.strip_prefix("Co-authored-by: ") else {
            continue;
        };
        let Some((name, email)) = rest.rsplit_once(" <") else {
            continue;
        };
        let Some(email) = email.strip_suffix('>') else {
            continue;
        };

        if name.chars().count() <= 38
            && name
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == ' ')
            && !email.chars().any(char::is_whitespace)
        {
            coauthors.push(format!("{name} <{email}>"));
        }
    }

    coauthors
}

fn preferred_name_format(name: &str) -> bool {
    let mut parts = name.split_whitespace();
    let Some(first_name) = parts.next() else {
        return false;
    };
    let Some(last_name) = parts.next_back() else {
        return false;
    };

    let mut chars = first_name.chars();
    let first_char_is_uppercase = chars.next().is_some_and(char::is_uppercase);
    first_char_is_uppercase
        && chars.all(char::is_lowercase)
        && last_name.chars().any(|ch| ch.is_alphabetic())
        && last_name
            .chars()
            .filter(|ch| ch.is_alphabetic())
            .all(char::is_uppercase)
}

fn email_matches_domain(email: &str, domain: Option<&str>) -> bool {
    let Some(domain) = domain else {
        return false;
    };
    let domain = domain.trim().trim_start_matches('@');
    email
        .rsplit_once('@')
        .is_some_and(|(_, email_domain)| email_domain.eq_ignore_ascii_case(domain))
}

fn compute_mailmap_rows(
    authors: impl IntoIterator<Item = String>,
    preferred_domain: Option<&str>,
) -> Vec<(String, String)> {
    let unique_authors: BTreeSet<String> = authors.into_iter().collect();
    let parsed: Vec<Author> = unique_authors
        .iter()
        .filter_map(|author| parse_author(author))
        .collect();

    let mut by_email: BTreeMap<String, Vec<Author>> = BTreeMap::new();
    for author in parsed {
        by_email
            .entry(author.email.clone())
            .or_default()
            .push(author);
    }

    let email_groups: Vec<Vec<Author>> = by_email.into_values().collect();
    let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for group in &email_groups {
        let names: BTreeSet<&str> = group.iter().map(|author| author.name.as_str()).collect();
        if names.len() == 1 {
            by_name
                .entry((*names.first().expect("non-empty group")).to_owned())
                .or_default()
                .extend(group.iter().map(|author| author.original.clone()));
        }
    }

    let mut groups: Vec<Vec<String>>;
    if by_name.is_empty() {
        groups = email_groups
            .iter()
            .map(|group| group.iter().map(|author| author.original.clone()).collect())
            .collect();
    } else {
        for group in &email_groups {
            let names: BTreeSet<&str> = group.iter().map(|author| author.name.as_str()).collect();
            if names.len() <= 1 {
                continue;
            }
            let originals: Vec<String> =
                group.iter().map(|author| author.original.clone()).collect();
            for name in names {
                if let Some(same_name_group) = by_name.get_mut(name) {
                    same_name_group.extend(originals.iter().cloned());
                }
            }
        }
        groups = by_name.into_values().collect();
    }

    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    for mut group in groups.drain(..) {
        group.sort();
        group.dedup();
        let Some(canonical_index) = group
            .iter()
            .enumerate()
            .min_by_key(|(_, identity)| {
                let author = parse_author(identity);
                let domain_match = author
                    .as_ref()
                    .is_some_and(|author| email_matches_domain(&author.email, preferred_domain));
                let preferred_name = author
                    .as_ref()
                    .is_some_and(|author| preferred_name_format(&author.name));
                (!domain_match, !preferred_name, identity.as_str())
            })
            .map(|(index, _)| index)
        else {
            continue;
        };
        let canonical = group.remove(canonical_index);
        let aliases: Vec<String> = if group.is_empty() {
            vec![String::new()]
        } else {
            group
        };

        for alias in aliases {
            if seen.insert((canonical.clone(), alias.clone())) {
                rows.push((canonical.clone(), alias));
            }
        }
    }
    rows
}

/// Generate suggested `.mailmap` contents from one or more Git repositories.
pub fn create_mailmap<P: AsRef<Path>>(paths_to_repos: &[P]) -> io::Result<String> {
    create_mailmap_with_domain(paths_to_repos, None)
}

/// Generate `.mailmap` contents, preferring identities on `preferred_domain`.
pub fn create_mailmap_with_domain<P: AsRef<Path>>(
    paths_to_repos: &[P],
    preferred_domain: Option<&str>,
) -> io::Result<String> {
    let mut authors = Vec::new();
    for path in paths_to_repos {
        let path = path.as_ref();
        authors.extend(get_authors_from_git(path)?);
        authors.extend(get_coauthors_from_git(path)?);
    }
    Ok(mailmap_rows_to_string(compute_mailmap_rows(
        authors,
        preferred_domain,
    )))
}

fn mailmap_rows_to_string(rows: Vec<(String, String)>) -> String {
    let mut output = String::new();
    for (author, alias) in rows {
        output.push_str(&author);
        output.push(' ');
        output.push_str(&alias);
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn identities(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn same_email_maps_name_variants_to_first_sorted_identity() {
        let rows = compute_mailmap_rows(
            identities(&[
                "Alice Smith <alice@example.com>",
                "A. Smith <alice@example.com>",
            ]),
            None,
        );
        assert_eq!(
            rows,
            vec![(
                "A. Smith <alice@example.com>".to_owned(),
                "Alice Smith <alice@example.com>".to_owned()
            )]
        );
    }

    #[test]
    fn same_name_maps_different_emails() {
        let rows = compute_mailmap_rows(
            identities(&[
                "Alice Smith <alice@work.example>",
                "Alice Smith <alice@home.example>",
            ]),
            None,
        );
        assert_eq!(
            rows,
            vec![(
                "Alice Smith <alice@home.example>".to_owned(),
                "Alice Smith <alice@work.example>".to_owned()
            )]
        );
    }

    #[test]
    fn preferred_domain_selects_canonical_email() {
        let rows = compute_mailmap_rows(
            identities(&[
                "Alice Smith <alice@personal.example>",
                "Alice Smith <alice@work.example>",
            ]),
            Some("WORK.EXAMPLE"),
        );
        assert_eq!(
            rows,
            vec![(
                "Alice Smith <alice@work.example>".to_owned(),
                "Alice Smith <alice@personal.example>".to_owned()
            )]
        );
    }

    #[test]
    fn preferred_name_style_selects_canonical_identity() {
        let rows = compute_mailmap_rows(
            identities(&[
                "MAYA Stone <maya@example.com>",
                "Maya STONE <maya@example.com>",
            ]),
            None,
        );
        assert_eq!(
            rows,
            vec![(
                "Maya STONE <maya@example.com>".to_owned(),
                "MAYA Stone <maya@example.com>".to_owned()
            )]
        );
    }

    #[test]
    fn domain_and_name_preferences_select_canonical_identity() {
        let rows = compute_mailmap_rows(
            identities(&[
                "MAYA Stone <maya@corp.example>",
                "Maya STONE <maya@corp.example>",
                "MAYA Stone <maya@personal.example>",
            ]),
            Some("corp.example"),
        );
        assert_eq!(
            rows,
            vec![
                (
                    "Maya STONE <maya@corp.example>".to_owned(),
                    "MAYA Stone <maya@corp.example>".to_owned()
                ),
                (
                    "Maya STONE <maya@corp.example>".to_owned(),
                    "MAYA Stone <maya@personal.example>".to_owned()
                )
            ]
        );
    }

    #[test]
    fn unrelated_single_identity_retains_empty_alias_field() {
        assert_eq!(
            mailmap_rows_to_string(compute_mailmap_rows(
                identities(&["Solo Person <solo@example.com>",]),
                None
            )),
            "Solo Person <solo@example.com> \n"
        );
    }

    #[test]
    fn coauthor_parser_matches_trailer_shape_and_name_limit() {
        let accepted =
            "Co-authored-by: First Last <first@example.com>\nCo-authored-by:  <empty@example.com>";
        assert_eq!(
            parse_coauthors(accepted),
            vec![
                "First Last <first@example.com>".to_owned(),
                " <empty@example.com>".to_owned()
            ]
        );

        let too_long_name = format!("Co-authored-by: {} <long@example.com>", "a".repeat(39));
        let rejected = format!(
            " Co-authored-by: Indented <indent@example.com>\nco-authored-by: Lowercase <lower@example.com>\nCo-authored-by: Bad<Name <bad@example.com>\n{too_long_name}\nCo-authored-by: Bad Email <bad email@example.com>"
        );
        assert!(parse_coauthors(&rejected).is_empty());
    }

    #[test]
    fn multiname_email_group_joins_matching_name_group() {
        let rows = compute_mailmap_rows(
            identities(&[
                "Alice Smith <shared@example.com>",
                "A. Smith <shared@example.com>",
                "Alice Smith <alice@example.com>",
            ]),
            None,
        );
        assert_eq!(
            rows,
            vec![
                (
                    "A. Smith <shared@example.com>".to_owned(),
                    "Alice Smith <alice@example.com>".to_owned()
                ),
                (
                    "A. Smith <shared@example.com>".to_owned(),
                    "Alice Smith <shared@example.com>".to_owned()
                )
            ]
        );
    }

    #[test]
    fn collects_commit_authors_and_declared_coauthors() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("mailmap-generator-test-{unique}"));
        fs::create_dir_all(&repo).unwrap();
        let run_git = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success());
        };

        run_git(&["init", "-q"]);
        run_git(&["config", "user.name", "Commit Author"]);
        run_git(&["config", "user.email", "author@example.com"]);
        fs::write(repo.join("file"), "contents").unwrap();
        run_git(&["add", "file"]);
        run_git(&[
            "commit",
            "-q",
            "-m",
            "initial",
            "-m",
            "Co-authored-by: Co Author <co@example.com>",
        ]);

        let output = create_mailmap(&[repo.as_path()]).unwrap();
        assert!(output.contains("Commit Author <author@example.com> \n"));
        assert!(output.contains("Co Author <co@example.com> \n"));

        fs::remove_dir_all(repo).unwrap();
    }
}
