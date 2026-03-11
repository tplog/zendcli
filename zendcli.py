#!/usr/bin/env python3
"""zendcli - Minimal Zendesk CLI for tickets and comments.

Wraps two Zendesk Support API endpoints:
  - GET /api/v2/tickets          (list tickets)
  - GET /api/v2/tickets/{id}/comments  (list ticket comments/thread)

Credentials are stored locally in ~/.zendcli/config.json.
Authentication uses Zendesk API token auth: {email}/token + api_token.
"""

import json
import os
import sys

import click
import requests

# Local config directory and file path
CONFIG_DIR = os.path.expanduser("~/.zendcli")
CONFIG_FILE = os.path.join(CONFIG_DIR, "config.json")


def load_config():
    """Load saved credentials from config file. Returns empty dict if not found."""
    if os.path.exists(CONFIG_FILE):
        with open(CONFIG_FILE) as f:
            return json.load(f)
    return {}


def save_config(config):
    """Persist credentials to ~/.zendcli/config.json."""
    os.makedirs(CONFIG_DIR, exist_ok=True)
    with open(CONFIG_FILE, "w") as f:
        json.dump(config, f, indent=2)


def get_config():
    """Read and validate credentials. Exits if not configured."""
    config = load_config()
    subdomain = config.get("subdomain")
    email = config.get("email")
    token = config.get("api_token")
    if not all([subdomain, email, token]):
        click.echo("Not configured. Run: zend configure", err=True)
        sys.exit(1)
    return subdomain, email, token


def api_get(path, params=None):
    """Make an authenticated GET request to the Zendesk API."""
    subdomain, email, token = get_config()
    url = f"https://{subdomain}.zendesk.com{path}"
    # Zendesk token auth format: {email}/token as username, api_token as password
    resp = requests.get(url, auth=(f"{email}/token", token), params=params)
    if resp.status_code != 200:
        click.echo(f"Error {resp.status_code}: {resp.text}", err=True)
        sys.exit(1)
    return resp.json()


@click.group()
def cli():
    """zendcli - Zendesk tickets & comments CLI."""
    pass


@cli.command()
def configure():
    """Set up Zendesk credentials interactively."""
    config = load_config()
    click.echo("Zendesk CLI Configuration")
    click.echo("─" * 30)
    # Show existing values as defaults so users can update selectively
    subdomain = click.prompt("Subdomain (xxx.zendesk.com)", default=config.get("subdomain", ""))
    email = click.prompt("Email", default=config.get("email", ""))
    api_token = click.prompt("API Token", default=config.get("api_token", ""), hide_input=True)

    save_config({"subdomain": subdomain, "email": email, "api_token": api_token})
    click.echo(f"\nSaved to {CONFIG_FILE}")


@cli.command()
@click.option("--status", type=click.Choice(["new", "open", "pending", "hold", "solved", "closed"]), help="Filter by status")
@click.option("--limit", default=20, show_default=True, help="Max tickets to return")
@click.option("--sort", "sort_order", type=click.Choice(["asc", "desc"]), default="desc", show_default=True)
def tickets(status, limit, sort_order):
    """List tickets from Zendesk. Sorted by updated_at by default."""
    params = {"per_page": min(limit, 100), "sort_order": sort_order, "sort_by": "updated_at"}
    data = api_get("/api/v2/tickets.json", params)

    results = data.get("tickets", [])
    # Client-side filter since the API doesn't support status as a query param
    if status:
        results = [t for t in results if t.get("status") == status]

    for t in results[:limit]:
        click.echo(f"[{t['id']}] {t['status']:<8} {t.get('subject', '(no subject)')}")
        click.echo(f"    priority={t.get('priority', '-')}  type={t.get('type', '-')}  created={t['created_at']}")
        click.echo()


@cli.command()
@click.argument("ticket_id", type=int)
@click.option("--type", "comment_type", type=click.Choice(["all", "public", "internal"]), default="all", show_default=True, help="Filter comment type")
@click.option("--sort", "sort_order", type=click.Choice(["asc", "desc"]), default="asc", show_default=True)
@click.option("--json-output", is_flag=True, help="Output raw JSON")
def comments(ticket_id, comment_type, sort_order, json_output):
    """List comments/thread for a ticket.

    Each comment has a 'public' flag:
      - public=true  -> customer-facing reply
      - public=false -> internal note (agents only)
    """
    params = {"sort_order": sort_order, "per_page": 100}
    data = api_get(f"/api/v2/tickets/{ticket_id}/comments.json", params)

    results = data.get("comments", [])
    # Filter by visibility type
    if comment_type == "public":
        results = [c for c in results if c.get("public") is True]
    elif comment_type == "internal":
        results = [c for c in results if c.get("public") is False]

    if json_output:
        click.echo(json.dumps(results, indent=2, ensure_ascii=False))
        return

    for c in results:
        label = "PUBLIC" if c.get("public") else "INTERNAL"
        via = c.get("via", {}).get("channel", "?")
        click.echo(f"--- [{label}] id={c['id']}  author={c.get('author_id')}  via={via}  {c['created_at']} ---")
        # Prefer plain_body for cleaner terminal output
        body = c.get("plain_body") or c.get("body") or ""
        click.echo(body.strip())
        click.echo()


if __name__ == "__main__":
    cli()
