---
name: zendcli
description: "Use the `zend` CLI for agent-first Zendesk ticket lookup. Trigger when the user wants to inspect tickets or comments from the terminal by ticket ID, assignee email, or follower email."
metadata:
  {
    "openclaw":
      {
        "emoji": "🎫",
        "requires": { "bins": ["zend"] },
        "install":
          [
            {
              "id": "npm",
              "kind": "npm",
              "package": "@tplog/zendcli",
              "bins": ["zend"],
              "label": "Install zendcli from npm"
            }
          ]
      }
  }
---

# Zendcli Skill

Use the `zend` CLI to access Zendesk tickets and comments from the terminal.

This skill reflects the **v2 refactor** of `zendcli`:
- agent-first usage
- JSON output by default
- argv routing based on argument shape
- assignee-based email lookup
- follower lookup built into the CLI

## When to Use

✅ **USE this skill when:**

- Looking up a single ticket by numeric ID
- Looking up tickets assigned to a Zendesk user email
- Looking up tickets where a Zendesk user is a follower
- Reading a ticket comment thread
- Doing quick support triage without opening the Zendesk web UI
- You want machine-readable JSON output for downstream summarization or analysis

## When NOT to Use

❌ **DON'T use this skill when:**

- Creating, editing, assigning, or closing tickets
- Managing Zendesk admin settings
- Accessing Zendesk resources other than tickets, users search for follower resolution, and comments
- Doing workflows that need browser-only context or UI actions
- Credentials are not configured and the user does not want interactive setup

## Supported Commands

```bash
zend <id> [--raw]
zend <email> [--status <status>] [--limit <n>] [--sort asc|desc]
zend follower <email> [--status <status>] [--limit <n>] [--sort asc|desc]
zend comments <ticketId> [--visibility all|public|private] [--sort asc|desc]
zend configure
```

## Routing Rules

The CLI preprocesses argv before `program.parse()`:

- `zend 12345` → route to ticket lookup
- `zend user@example.com` → route to assignee-email lookup
- `zend follower user@example.com` → explicit follower lookup
- known subcommands like `configure`, `comments`, `follower`, `help` are left unchanged

## Output Rules

- Normal output is JSON on `stdout`
- Error output is also JSON on `stdout`
- Errors exit with non-zero code
- Do **not** rely on `--json`; v2 output is already JSON-first

Example error shape:

```json
{"error":"not_found","message":"Ticket 99999 not found","id":99999}
```

## Setup

Install:

```bash
npm install -g @tplog/zendcli
```

Preferred authentication for agent workflows:

```bash
export ZENDESK_SUBDOMAIN="your-subdomain"
export ZENDESK_EMAIL="you@example.com"
export ZENDESK_API_TOKEN="your_zendesk_api_token"
```

Interactive setup is still available:

```bash
zend configure
```

The CLI may also store credentials in:

```bash
~/.zendcli/config.json
```

Authentication uses Zendesk API token auth with `{email}/token` as the Basic auth username.

## Common Usage

### Check commands first

```bash
zend --help
zend follower --help
zend comments --help
```

### Get one ticket

```bash
zend 12345
```

By default, output is slimmed to key fields (id, subject, description, status, priority, dates, tags, assignee/requester IDs, etc.) to save context. Use `--raw` for the full Zendesk API response:

```bash
zend 12345 --raw
```

### Get tickets assigned to an email

```bash
zend user@example.com
zend user@example.com --status unresolved
zend user@example.com --status open,pending
zend user@example.com --limit 10
zend user@example.com --sort asc
```

Semantics:
- this is **assignee** lookup in v2
- not requester lookup

### Get follower tickets

```bash
zend follower user@example.com
zend follower user@example.com --status open
zend follower user@example.com --limit 5
```

Semantics:
- resolves the Zendesk user by email
- searches tickets
- filters client-side to `follower_ids.includes(user.id) && assignee_id !== user.id`

### Read comments

```bash
zend comments 12345
zend comments 12345 --visibility public
zend comments 12345 --visibility private
```

Default output is a slim timeline optimized for summarization:

```json
[
  {
    "author": "Support Agent",
    "time": "2026-03-13T06:19:57Z",
    "visibility": "public",
    "body": "Reply text..."
  }
]
```

## API Mapping

The current CLI uses these Zendesk APIs:

- `GET /api/v2/tickets/{id}.json`
- `GET /api/v2/search.json?query=type:ticket assignee:{email}`
- `GET /api/v2/users/search.json?query={email}`
- `GET /api/v2/search.json` for follower traversal and filtering
- `GET /api/v2/tickets/{id}/comments.json`

## Practical Guidance

- If the user gives a numeric ticket ID, use `zend <id>`.
- If the user gives an email and wants "their tickets", use `zend <email>` and remember this means **assigned to that email** in v2.
- If the user explicitly asks for follower tickets, use `zend follower <email>`.
- If the user wants the thread for a known ticket, use `zend comments <id>`.
- Since output is already JSON, summarize directly from stdout instead of asking for a JSON flag.
- `zend comments <id>` returns a slim timeline with `author`, `time`, `visibility`, and `body`.
- By default, `zend comments <id>` includes both public and private comments; use `--visibility` to filter.

## Troubleshooting

### Command not found

Install the package globally and verify the binary is on `PATH`:

```bash
npm install -g @tplog/zendcli
which zend
```

### Auth or API errors

Prefer environment variables first. If needed, re-run:

```bash
zend configure
```

### Empty results

- Check whether the email is correct
- Remember `zend <email>` means **assignee**, not requester
- Try a different `--status`
- Increase `--limit`
- For follower lookup, verify the user exists in Zendesk

## Notes for the Agent

- Always start with `zend --help` before relying on command shapes.
- In v2, `zend <email>` is **assignee lookup**, not requester lookup.
- In v2, `zend <id>` is the fast path for single-ticket lookup.
- In v2, `zend follower <email>` exists and should be preferred over manual API fallback.
- Output is already JSON; do not assume human-readable output.
- If a command fails, inspect the JSON error on stdout instead of only stderr.
- If `zend` is unavailable, tell the user it can be installed with `npm install -g @tplog/zendcli`.
- When the user asks to **summarize** or **review** a ticket, **parallel call** both `zend <id>` and `zend comments <id>` in the same tool-call block — ticket metadata alone is not enough for a complete summary.
