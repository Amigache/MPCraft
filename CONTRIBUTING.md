# Contributing

## Language

The project is written in **English**: commit messages, pull-request titles and descriptions, code
and doc comments, documentation, test names, and file names. A sentence in another language is a
finding, because the code is read by people who do not share one.

The interface is the exception in the other direction: a source string is written once in English and
looked up in the translation bundles (`apps/ui/src/lib/i18n/strings.ts`), so the words the operator
reads can be another language without a second implementation. A string that is not in a bundle is
said as it is written, which keeps a missing translation a gap rather than a blank.

An existing passage in another language is translated when it is edited, not as a separate change,
and it is not copied into new text. This applies to `docs/PARITY.md` as much as to the code.

## The branches

| Branch | What it is |
| --- | --- |
| `main` | The released line. A tag is made from here. |
| `develop` | Where the changes meet. Everything lands here first. |
| `<area>/<topic>` | One change: `ui/plan-editor`, `link/heartbeat-timeout`, `docs/release` |

A change goes: **a branch from `develop` → a pull request to `develop` → `develop` → a pull request
to `main`**. Direct pushes to either branch are refused by the repository, and a pull request to
`main` that does not come from `develop` is refused by the check `The flow of the branches`.

A release is tagged from `main` after `develop` has been merged into it. The steps are in
[`docs/RELEASE.md`](docs/RELEASE.md).

## Before opening a pull request

Run what the workflow runs, from the repository root:

```bash
node scripts/run-gates.js
```

That covers the Rust formatting, the lints, the tests, the frontend type-check, lint, tests and
Playwright flows, the contract, the parity matrix, the launch and the packaging. A layer that was not
touched is reported as `NO-APLICA` rather than counted as a pass.

The checks a pull request must satisfy are:

- `Rust / ubuntu-latest`, `Rust / windows-latest`, `Rust / macos-latest`
- `Frontend`
- `SITL integration`
- `The flow of the branches`

The packaging job also runs; it is not required, because it builds three installers and takes
longer than a review.

## What a change carries

- A test for what it does, next to the code it tests. A test must assert something: calling code is
  not a test.
- The row of [`docs/PARITY.md`](docs/PARITY.md) it changes, in the same change. No row is left
  blank, and a partial row says what is missing.
- A commit message that says **why**, in the language the repository speaks: the rule it follows, the
  fact that corrected it, and what it does not cover.
- No key, password, token or certificate. The private key of the updater lives outside the
  repository; see [`docs/RELEASE.md`](docs/RELEASE.md).

## Security

A vulnerability is reported privately, not as a pull request. See [`SECURITY.md`](SECURITY.md).
