# Security policy

MPCraft is a ground control station: it can arm a vehicle, write its plan and erase its flash. A
defect that lets somebody else do any of those things is treated as a security issue.

## Supported versions

The latest release on the `main` branch is the supported one. Fixes are made on `develop` and reach
`main` through a pull request, and a new release is tagged from `main`.

## Reporting a vulnerability

**Do not open a public issue for a vulnerability.** Use GitHub's private reporting: the **Security**
tab of the repository → **Report a vulnerability**. That opens a private advisory that only the
maintainers can read.

Please include:

- the version and the platform, and whether the copy is a release or a build from source;
- what happens and what should happen instead;
- the smallest reproduction you have, and what an attacker gains.

If private reporting is not available to you, open an issue that says only that you have a security
report and asks for a private channel; leave the details out of it.

## What is in scope

- the application and its command surface (`apps/`, `crates/mpcraft-app`);
- the link, the protocol and the transports (`crates/mpcraft-link`, `mpcraft-mavlink`,
  `mpcraft-transport`);
- the plugin sandbox and its capabilities (`crates/mpcraft-plugin`);
- the update path: the release, the manifest and the signature (`docs/RELEASE.md`);
- the file formats and the parsers.

## What is out of scope

- the firmware and the bootloader of a board, which are not part of this project;
- the services the operator chooses for tiles, terrain or traffic, and what they do with a request;
- a machine that is already compromised, or an attacker who can read the operator's files;
- denial of service that needs the vehicle or the network to be flooded beyond what a link carries.

## What to expect

We aim to acknowledge a report within seven days, to keep you informed while it is fixed, and to
credit you in the advisory unless you prefer otherwise. Please give us a reasonable time to release a
fix before saying anything in public.
