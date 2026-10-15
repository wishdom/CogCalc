# Contributing to CogCalc

Bug reports, documentation improvements, and code contributions are welcome.

- Use [Discussions](https://github.com/wishdom/CogCalc/discussions) for questions, setup help, and early ideas.
- Use [issues](https://github.com/wishdom/CogCalc/issues) for reproducible bugs and concrete feature requests. Search existing issues and discussions first.
- Report vulnerabilities privately using the process in [SECURITY.md](SECURITY.md).

## Making a change

1. Fork the repository and create a branch from `main`.
2. Follow the [build instructions](README.md#build-and-run). Rust 1.95 or newer and the platform's native build prerequisites are required.
3. Keep the change focused. For substantial changes, describe the proposal in an issue or discussion so contributors can discuss the approach.
4. Update relevant documentation and add or adjust tests when behavior changes.
5. Open a pull request against `main`, describing the problem, the change, and how you verified it. Link any related issue.

For Rust changes, run the same checks used by CI:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Run `cargo fmt --all` to apply formatting. For interface changes, also run `cargo run --locked` and exercise the affected controls; screenshots help reviewers assess visual changes. Include your operating system and any checks you could not run in the pull request. Documentation-only changes do not need a Rust build.

The [Development section](README.md#development) describes the source layout and existing tests. CI checks Windows, Linux, and macOS; contributors can verify locally on their available platform. Pull requests must pass all three platform checks, be up to date with `main`, and resolve review conversations before merging.

## License

Contributions use the project's existing [GPL-3.0-only license](LICENSE). Only submit material you have the right to contribute, and preserve required attribution and license notices.
