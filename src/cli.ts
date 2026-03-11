/**
 * zendcli - Minimal Zendesk CLI for tickets and comments.
 *
 * Wraps two Zendesk Support API endpoints:
 *   - GET /api/v2/tickets              (list tickets)
 *   - GET /api/v2/tickets/{id}/comments (list ticket comments/thread)
 */

import { Command } from "commander";
import * as readline from "readline";
import { loadConfig, saveConfig } from "./config";
import { apiGet } from "./api";

const program = new Command();
const VALID_TICKET_STATUSES = ["new", "open", "pending", "hold", "solved", "closed"];

program.name("zend").description("Zendesk tickets & comments CLI").version("0.1.0");

/** Interactive prompt helper. */
function prompt(question: string, defaultValue = ""): Promise<string> {
  return new Promise((resolve) => {
    const rl = readline.createInterface({ input: process.stdin, output: process.stderr });
    const suffix = defaultValue ? ` [${defaultValue}]` : "";
    rl.question(`${question}${suffix}: `, (answer) => {
      rl.close();
      resolve(answer || defaultValue);
    });
  });
}

function promptHidden(question: string, hasDefault = false): Promise<string> {
  return new Promise((resolve) => {
    const stdin = process.stdin;
    const stdout = process.stderr;
    let value = "";

    readline.emitKeypressEvents(stdin);
    if (stdin.isTTY) {
      stdin.setRawMode(true);
    }

    const suffix = hasDefault ? " [****]" : "";
    stdout.write(`${question}${suffix}: `);

    const onKeypress = (char: string, key: readline.Key) => {
      if (key.name === "return" || key.name === "enter") {
        stdout.write("\n");
        stdin.off("keypress", onKeypress);
        if (stdin.isTTY) {
          stdin.setRawMode(false);
        }
        resolve(value);
        return;
      }

      if (key.ctrl && key.name === "c") {
        stdout.write("\n");
        stdin.off("keypress", onKeypress);
        if (stdin.isTTY) {
          stdin.setRawMode(false);
        }
        process.exit(130);
      }

      if (key.name === "backspace") {
        value = value.slice(0, -1);
        return;
      }

      if (char) {
        value += char;
      }
    };

    stdin.on("keypress", onKeypress);
  });
}

function parseStatusFilter(input?: string): string[] {
  if (!input) return [];
  const statuses = input.split(",").map((value) => value.trim().toLowerCase()).filter(Boolean);
  const invalid = statuses.filter((status) => status !== "unresolved" && status !== "all" && !VALID_TICKET_STATUSES.includes(status));

  if (invalid.length > 0) {
    console.error(`Invalid status value(s): ${invalid.join(", ")}`);
    console.error(`Allowed values: unresolved, all, ${VALID_TICKET_STATUSES.join(", ")}`);
    process.exit(1);
  }

  if (statuses.includes("all")) return [];
  if (statuses.includes("unresolved")) return ["new", "open", "pending", "hold"];
  return statuses;
}

// --- configure ---
program
  .command("configure")
  .description("Set up Zendesk credentials interactively")
  .action(async () => {
    const existing = loadConfig();
    console.error("Zendesk CLI Configuration");
    console.error("─".repeat(30));

    const subdomain = await prompt("Subdomain (xxx.zendesk.com)", existing.subdomain);
    const email = await prompt("Email", existing.email);
    const tokenInput = await promptHidden("API Token", Boolean(existing.api_token));
    const api_token = tokenInput || existing.api_token || "";

    saveConfig({ subdomain, email, api_token });
    console.error("\nSaved to ~/.zendcli/config.json with restricted permissions (0600)");
    console.error("Environment variables also work: ZENDESK_SUBDOMAIN, ZENDESK_EMAIL, ZENDESK_API_TOKEN");
  });

// --- tickets ---
program
  .command("tickets")
  .description("List tickets from Zendesk, sorted by updated_at")
  .option("--status <status>", "Filter by status (new|open|pending|hold|solved|closed)")
  .option("--limit <n>", "Max tickets to return", "20")
  .option("--sort <order>", "Sort order (asc|desc)", "desc")
  .action(async (opts) => {
    const limit = parseInt(opts.limit, 10);
    const data = await apiGet<{ tickets: any[] }>("/api/v2/tickets.json", {
      per_page: Math.min(limit, 100),
      sort_order: opts.sort,
      sort_by: "updated_at",
    });

    let tickets = data.tickets || [];
    if (opts.status) {
      tickets = tickets.filter((t) => t.status === opts.status);
    }

    for (const t of tickets.slice(0, limit)) {
      const status = (t.status || "").padEnd(8);
      const subject = t.subject || "(no subject)";
      console.log(`[${t.id}] ${status} ${subject}`);
      console.log(`    priority=${t.priority ?? "-"}  type=${t.type ?? "-"}  created=${t.created_at}`);
      console.log();
    }
  });

// --- email ---
program
  .command("email <email>")
  .description("Find tickets for a requester email")
  .option("--status <status>", "Filter status: unresolved|all|new|open|pending|hold|solved|closed or comma-separated list", "unresolved")
  .option("--limit <n>", "Max tickets to return", "20")
  .option("--sort <order>", "Sort order (asc|desc)", "desc")
  .option("--json", "Output raw JSON")
  .action(async (email, opts) => {
    const limit = parseInt(opts.limit, 10);
    const statusFilter = parseStatusFilter(opts.status);
    const canUseSearchStatus = opts.status === "unresolved" || statusFilter.length === 1;
    let query = `type:ticket requester:${email}`;

    if (opts.status === "unresolved") {
      query += " status<solved";
    } else if (canUseSearchStatus && statusFilter.length === 1) {
      query += ` status:${statusFilter[0]}`;
    }

    const data = await apiGet<{ results: any[] }>("/api/v2/search.json", {
      query,
      sort_by: "updated_at",
      sort_order: opts.sort,
      per_page: 100,
    });

    let tickets = data.results || [];
    if (statusFilter.length > 0 && opts.status !== "unresolved" && !canUseSearchStatus) {
      tickets = tickets.filter((ticket) => statusFilter.includes((ticket.status || "").toLowerCase()));
    }

    tickets = tickets.slice(0, limit);

    if (opts.json) {
      console.log(JSON.stringify(tickets, null, 2));
      return;
    }

    for (const t of tickets) {
      const status = (t.status || "").padEnd(8);
      const subject = t.subject || "(no subject)";
      console.log(`[${t.id}] ${status} ${subject}`);
      console.log(`    priority=${t.priority ?? "-"}  type=${t.type ?? "-"}  requester=${email}  updated=${t.updated_at}`);
      console.log();
    }
  });

// --- comments ---
program
  .command("comments <ticketId>")
  .description("List comments/thread for a ticket (public=customer-facing, internal=agents only)")
  .option("--type <type>", "Filter: all|public|internal", "all")
  .option("--sort <order>", "Sort order (asc|desc)", "asc")
  .option("--json", "Output raw JSON")
  .action(async (ticketId, opts) => {
    const data = await apiGet<{ comments: any[] }>(
      `/api/v2/tickets/${ticketId}/comments.json`,
      { sort_order: opts.sort, per_page: 100 }
    );

    let comments = data.comments || [];
    if (opts.type === "public") {
      comments = comments.filter((c) => c.public === true);
    } else if (opts.type === "internal") {
      comments = comments.filter((c) => c.public === false);
    }

    if (opts.json) {
      console.log(JSON.stringify(comments, null, 2));
      return;
    }

    for (const c of comments) {
      const label = c.public ? "PUBLIC" : "INTERNAL";
      const via = c.via?.channel ?? "?";
      console.log(`--- [${label}] id=${c.id}  author=${c.author_id}  via=${via}  ${c.created_at} ---`);
      const body = (c.plain_body || c.body || "").trim();
      console.log(body);
      console.log();
    }
  });

program.parse();
