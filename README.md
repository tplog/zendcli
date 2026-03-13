# zendcli

Minimal Zendesk CLI for listing tickets and reading ticket comment threads.

## Install

```bash
npm install -g @tplog/zendcli
```

## Configure

### Option 1: interactive setup

```bash
zend configure
```

Credentials are stored locally in:

```bash
~/.zendcli/config.json
```

The CLI writes this file with restricted permissions.

### Option 2: environment variables

This is the recommended option for temporary or CI usage.

```bash
export ZENDESK_SUBDOMAIN="your-subdomain"
export ZENDESK_EMAIL="user@example.com"
export ZENDESK_API_TOKEN="your_zendesk_api_token"
```

Environment variables take precedence over the config file.

## Usage

```bash
zend --help
zend tickets --help
zend email --help
zend follower --help
zend comments --help
zend tickets --limit 10
zend tickets --status open --limit 20
zend email user@example.com
zend email user@example.com --status unresolved
zend email user@example.com --status open,pending
zend follower
zend follower user@example.com --limit 3
zend comments 12345
zend comments 12345 --type public
zend comments 12345 --json
```

## Development workflow

### Daily development

1. Create a feature branch from `main`
2. Develop locally
3. Commit as needed
4. Push branch to GitHub
5. Open a PR to `main`
6. Merge after CI passes

Example:

```bash
git checkout main
git pull
git checkout -b feat/some-change

# work locally
npm ci
npm run build

git add .
git commit -m "feat: add some change"
git push -u origin feat/some-change
```

## Release workflow

This repository uses a safer release flow:

- normal merges to `main` do **not** publish automatically
- npm publishing happens only when a version tag is pushed
- the release tag must match `package.json` version exactly

### Publish a new version

1. Make sure `main` is in the state you want to release
2. Bump the version locally
3. Push `main` and the new tag
4. GitHub Actions publishes to npm

```bash
git checkout main
git pull
npm version patch
git push origin main --tags
```

Or for a feature release:

```bash
npm version minor
git push origin main --tags
```

This creates tags like `v0.1.2`, and the publish workflow verifies that the tag matches `package.json`.

## CI/CD

- `CI`: runs on branch pushes, PRs to `main`, and pushes to `main`
- `Publish to npm`: runs only on `v*` tags or manual trigger

## Trusted publishing

The publish workflow is set up for npm trusted publishing via GitHub Actions OIDC.

Recommended setup on npm:

1. Go to the package settings on npm
2. Add a Trusted Publisher for GitHub Actions
3. Point it to:
   - owner: `tplog`
   - repo: `zendcli`
   - workflow file: `publish.yml`

This avoids storing long-lived npm tokens in the repository.

## Security notes

- Never commit real Zendesk credentials
- Prefer environment variables for temporary use
- If a token is ever exposed, revoke and rotate it immediately
- Do not store npm publish credentials in the repo or in gitignored files
