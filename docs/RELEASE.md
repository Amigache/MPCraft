# Releases and updates

MPCraft publishes its releases on GitHub, and the application updates itself from them. The
installers, the archives the updater installs and their signatures are assets of one release; the
document that says which archive belongs to which platform is attached to the same release as
`latest.json`.

## What only the operator can supply

Two things are not in the repository:

| Secret | What it is |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | The private half of the key the updater verifies with |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password, if it has one |

They go in **Settings → Secrets and variables → Actions → New repository secret**. A copy of the
private key generated for this repository was written outside it, at
`%USERPROFILE%\mpcraft-release\updater.key` on the machine that generated it; move it somewhere safe
and add its **contents** as the first secret. The public half is committed in
`apps/desktop/tauri.conf.json`, under `plugins.updater.pubkey`.

Losing the private key means no further update can be verified by a copy already installed. Leaking
it means anybody can sign one. Neither is recoverable from the repository.

## Making a release

1. On `develop`, set the same version in `apps/desktop/tauri.conf.json` and in the workspace
   (`Cargo.toml`), for example `0.2.0`.
2. Open a pull request from `develop` to `main` and merge it once the checks pass.
3. On `main`, tag `v0.2.0` and push the tag.
4. The `bundle` job builds Windows, Linux and macOS with `tauri.updater.conf.json`, which turns the
   update archives on; it signs them, publishes the assets, and leaves one fragment of the manifest
   per platform. `publish-manifest` merges the fragments and attaches `latest.json`.
5. Check the release page: it carries the installers, the update bundles (the NSIS `.exe` or the
   MSI on Windows, the `.AppImage` on Linux, the `.app.tar.gz` on macOS), their `.sig` files and
   `latest.json`.

A change that is not a tag builds the installers only: the update artifacts are off in the main
configuration, so a pull request needs no key and cannot publish anything.

## What the application does

**Help → Check for updates…** asks the release, and says whether this copy is current. If a newer
version is announced, it asks for confirmation before downloading it, verifies the archive against
the public key, installs it and starts the application again. Nothing is checked at launch.

## Limits, said rather than hidden

- On Linux the updater installs the **AppImage**. A copy installed from deb or rpm is updated by the
  package manager, and the manifest still names the AppImage for that platform.
- The macOS runner builds the architecture it runs on — today arm64. An Intel Mac needs an entry of
  its own (`darwin-x86_64`) from a runner of that architecture.
- The release is public: the updater does not authenticate. A private repository would need a public
  place for the assets or a host of its own.
- `latest.json` is attached after the installers, so for a moment a release exists without it; a
  check in that window says there is nothing newer rather than failing.
