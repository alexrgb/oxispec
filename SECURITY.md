# Security policy

## Supported versions

OxiSpec is pre-1.0. Only the latest released version receives security fixes.

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Use GitHub's private reporting instead: open the **Security** tab of this repository, choose **Report a vulnerability**, and describe the problem. Include:

- the version (`oxispec --version`) and your operating system
- the steps or input files that trigger it
- what you expected, and what happens instead

We aim to acknowledge a report within 7 days and to tell you what we plan to do within 30 days. We will credit you in the release notes unless you prefer not to be named.

## Scope

In scope: the `oxispec` command line and the `oxispec-core` library, for example path handling that escapes the project folder, unsafe file writes, or crashes on crafted input.

Out of scope: problems in third-party dependencies (report those upstream, and tell us if we should update), and issues that need an already compromised machine.
