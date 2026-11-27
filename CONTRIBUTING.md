# Contributing

## Where work is tracked

GitHub Issues on this repository: https://github.com/Jodre11/pixel-factor/issues

Open an issue before starting anything larger than a small fix, so the approach can be agreed first. Use the issue templates.

To see what has been done: `git log`, and the closed pull requests.

## Proposing a change

1. Branch from `main`. Name the branch for the change, for example `service-skeleton` or `fix-offset-rounding`.
2. Run `make setup` once on a fresh checkout.
3. Make the change. Use `make check-changed` while iterating.
4. Run `make check` before pushing. CI runs the same command.
5. Open a pull request to `main` and fill in the template.

A pull request is ready to merge when CI is green and the code owner has approved it. Code owners are listed in `.github/CODEOWNERS`.

## Quality gates

`make check` is the gate. It covers formatting, Clippy with strict lints, unit and property tests, API docs, dependency policy and shell linting. [AGENTS.md](AGENTS.md) lists each gate and what it catches.

CI also runs `make coverage` on every pull request, and mutation testing, fuzzing and a vulnerability check weekly.

- New behaviour needs a test. Where there is an invariant, state it as a property test in `core/tests/properties.rs`.
- Coverage may not fall below the floors set by `COVERAGE_MIN` (core) and `SERVICE_COVERAGE_MIN` (service) in the `Makefile`. Raise the floor when coverage rises; do not lower it to pass.
- A surviving mutant in the weekly run means a test is missing. Add the test.
- These gates stop regressions. They do not show the design is secure. That needs human review.

## Commits

- Subject line in the imperative, 72 characters or fewer: "Add pepper rotation helper".
- Explain why in the body when it is not obvious.
- Commit under your own identity.
- No tool or AI attribution lines, such as `Co-Authored-By` trailers naming an assistant or "Generated with" footers, in commits or pull request descriptions. CI rejects them.

## What needs extra care

- Anything that changes the stored record format. See the gotchas in [AGENTS.md](AGENTS.md).
- Anything touching the principles in [docs/PRODUCT.md](docs/PRODUCT.md). State in the pull request which principle the change serves.
- New dependencies. The core has few on purpose. Justify each one.

## Security issues

Do not open a public issue. See [SECURITY.md](SECURITY.md).

## Licence

Contributions are accepted under the project's dual licence, MIT or Apache-2.0.
