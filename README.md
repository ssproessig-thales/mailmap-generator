# mailmap-generator
A Rust port of the mailmap generator tool from GitLab

Generate a suggested `.mailmap` from one or more Git repositories:

```sh
cargo run -- [--domain example.com] /path/to/repository > /path/to/repository/.mailmap
```

The `--domain` option (also available as `--domain=example.com`) prefers that
email domain when choosing a canonical identity. Within a group with equal
domain preference, names formatted as `FirstName LASTNAME` are preferred.
The `mailmap` executable also accepts `-h`/`--help` and `--version`. Git must
be available on `PATH`. Output is a suggestion and should be reviewed before
use: identities with the same email or exact name are grouped together, and
same-name grouping can accidentally combine different people.
