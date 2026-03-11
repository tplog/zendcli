#!/usr/bin/env bun
/**
 * zendcli - Minimal Zendesk CLI for tickets and comments.
 *
 * Wraps two Zendesk Support API endpoints:
 *   - GET /api/v2/tickets              (list tickets)
 *   - GET /api/v2/tickets/{id}/comments (list ticket comments/thread)
 */

import { Command } from "commander";
import { createInterface } from "readline/promises";
import { loadConfig, saveConfig } from "./config";
import { apiGet } from "./api";

const program = new Command();

program.name("zend").description("Zendesk tickets & comments CLI").version("0.1.0");

/** Interactive prompt helper. */
async function prompt(question: string, defaultValue = "", hidden = false): Promise<string> {
  const rl = createInterface({ input: process.stdin, output: process.stderr });
  const suffix = defaultValue ? ` [${hidden ? "****" : defaultValue}]` : "";
  const answer = await rl.question(`${question}${suffix}: `);
  rl.close();
  return answer || defaultValue;
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
    const api_token = await prompt("API Token", existing.api_token, true);

    saveConfig({ subdomain, email, api_token });
    console.error("\nSaved to ~/.zendcli/config.json");
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
    // Client-side filter since the API doesn't support status as a query param
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
    // Filter by visibility type
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
      // Prefer plain_body for cleaner terminal output
      const body = (c.plain_body || c.body || "").trim();
      console.log(body);
      console.log();
    }
  });

program.parse();
