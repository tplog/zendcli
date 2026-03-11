# Security and Release Decisions

## Goal

Make `zendcli` safe to open source and easy to maintain, while keeping the development and npm release workflow simple.

## Key Changes

### 1. Secured local credential handling in the CLI

Updated:
- `src/cli.ts`
- `src/config.ts`

Changes:
- Added real hidden input for `API Token` during `zend configure`
- Added support for environment variables:
  - `ZENDESK_SUBDOMAIN`
  - `ZENDESK_EMAIL`
  - `ZENDESK_API_TOKEN`
- Made environment variables override file-based config
- Restricted local credential storage permissions:
  - `~/.zendcli` directory: `0700`
  - `~/.zendcli/config.json` file: `0600`

Why:
- Token input was previously not truly hidden during interactive setup
- Users may prefer not to persist credentials to disk
- Restricting file permissions reduces local exposure risk
- Environment-variable support is a standard CLI security pattern and is better for temporary and CI use

### 2. Switched npm publishing from `main` push to tag-based release

Updated:
- `.github/workflows/publish.yml`

Changes:
- Publishing is no longer triggered by every push to `main`
- Publishing now runs only on:
  - pushed tags matching `v*`
  - manual workflow dispatch
- Added verification that git tag matches `package.json` version
- Added verification that the package version is not already published

Why:
- Publishing on every merge to `main` is convenient but too easy to trigger accidentally
- Development and release should be separate actions
- Tag-based publishing creates a clear release boundary
- Matching tag and version improves traceability and reduces release mistakes
- Preventing duplicate publishes avoids broken release runs

### 3. Adopted npm Trusted Publishing direction

Updated:
- `.github/workflows/publish.yml`
- `README.md`

Changes:
- Added `id-token: write` permission to the publish workflow
- Updated the workflow to use modern npm publishing with provenance
- Documented npm Trusted Publishing / OIDC setup in README

Why:
- Trusted Publishing is safer than storing long-lived npm tokens
- It avoids putting npm secrets in the repo, memory, or gitignored files
- It removes the need for manual 2FA/OTP during CI publishing
- It is the recommended approach for GitHub Actions → npm publishing

Note:
- The repository can remain private or public, but it has already been made public
- For private repositories, provenance may be limited, but publishing itself is supported

### 4. Improved repository metadata for open-source usage

Updated:
- `package.json`
- `README.md`

Changes:
- Added `repository`, `homepage`, and `bugs` fields to `package.json`
- Added README documentation for:
  - installation
  - configuration
  - environment variables
  - daily development flow
  - release flow
  - security notes

Why:
- Open-source packages should clearly document usage and release expectations
- npm metadata improves discoverability and trust
- Security expectations should be written down, not kept implicit

### 5. Enabled GitHub repository protection for secret exposure

Repository setting changes:
- Enabled secret scanning
- Enabled push protection

Why:
- Preventing secret leaks is better than cleaning them up afterward
- Push protection adds an extra guardrail before sensitive data lands in git history
- This is especially important after making the repository public

## Review of Existing Commit History

Before making the repository public, the git history was reviewed.

What was checked:
- recent commits
- full commit history for suspicious secret-like patterns
- specific commits that matched loose heuristics

Result:
- No real Zendesk API token or other confirmed secret was found in the reviewed commit history
- Pattern matches found during scanning were code variables like `api_token = click.prompt(...)`, not actual credentials

Why this mattered:
- A repository should not be made public until current files and commit history are both checked for credential exposure
- Removing a secret from the latest commit is not enough if it still exists in earlier commits

## Final Recommended Workflow

### Daily development
1. Create a feature branch from `main`
2. Develop locally
3. Commit normally
4. Push branch
5. Open a PR to `main`
6. Merge after CI passes

### Release
1. Ensure `main` is in a releasable state
2. Bump version with `npm version patch|minor|major`
3. Push `main` and tags
4. GitHub Actions publishes to npm from the tag

Example:

```bash
git checkout main
git pull
npm version patch
git push origin main --tags
```

Why this workflow was chosen:
- It keeps day-to-day development simple
- It avoids publishing by accident
- It creates an explicit and auditable release action
- It is easy to explain and maintain
- It aligns with secure npm publishing practices

## Security Rules Going Forward

- Never commit real Zendesk credentials
- Prefer environment variables for temporary use
- Do not store npm publish credentials in the repository
- Do not store 2FA codes, recovery codes, or publish tokens in memory files or gitignored project files
- If any token is ever suspected to be exposed, revoke and rotate it immediately
