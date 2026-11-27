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
