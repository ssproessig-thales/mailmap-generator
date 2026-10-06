# mailmap-generator
A Rust port of the mailmap generator tool from GitLab

Generate a suggested `.mailmap` from one or more Git repositories:

```sh
cargo run -- /path/to/repository > /path/to/repository/.mailmap
```

The `mailmap` executable also accepts `-h`/`--help` and `--version`. Git must
be available on `PATH`. Output is a suggestion and should be reviewed before
use: identities with the same email or exact name are grouped together, and
same-name grouping can accidentally combine different people.
