import { Command } from "commander";
import * as readline from "readline";
import { ApiError, apiGet, apiGetUrl } from "./api";
import { getConfig, loadConfig, saveConfig } from "./config";

const program = new Command();
const VALID_TICKET_STATUSES = ["new", "open", "pending", "hold", "solved", "closed"];
const KNOWN_COMMANDS = new Set(["configure", "follower", "comments", "help", "email", "ticket"]);
const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const DIGITS_RE = /^\d+$/;

type SearchTicket = Record<string, unknown> & {
  id: number;
  status?: string;
  assignee_id?: number | null;
  follower_ids?: number[];
};

type User = {
  id: number;
  email?: string;
};

class CliError extends Error {
  code: string;
  details: Record<string, unknown>;
  exitCode: number;

  constructor(code: string, message: string, details: Record<string, unknown> = {}, exitCode = 1) {
    super(message);
    this.name = "CliError";
    this.code = code;
    this.details = details;
    this.exitCode = exitCode;
  }
}

program.name("zend").description("Zendesk tickets CLI").version("2.0.0");

function printJson(value: unknown): void {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`);
}

function fail(code: string, message: string, details: Record<string, unknown> = {}, exitCode = 1): never {
  printJson({ error: code, message, ...details });
  process.exit(exitCode);
}

function handleError(error: unknown): never {
  if (error instanceof CliError) {
    fail(error.code, error.message, error.details, error.exitCode);
  }

  if (error instanceof ApiError) {
    if (error.status === 401) {
      fail("auth_failed", "401 Unauthorized", { status: 401 });
    }
    if (error.status === 404) {
      fail("not_found", "Resource not found", { status: 404 });
    }
    fail("api_error", error.body || error.message, error.status ? { status: error.status } : {});
  }

  const message = error instanceof Error ? error.message : "Unknown error";
  fail("unknown_error", message);
}

function run<T extends unknown[]>(fn: (...args: T) => Promise<void> | void) {
  return async (...args: T): Promise<void> => {
    try {
      await fn(...args);
    } catch (error) {
      handleError(error);
    }
  };
}

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
    if (stdin.isTTY) stdin.setRawMode(true);

    stdout.write(`${question}${hasDefault ? " [****]" : ""}: `);

    const onKeypress = (char: string, key: readline.Key) => {
      if (key.name === "return" || key.name === "enter") {
        stdout.write("\n");
        stdin.off("keypress", onKeypress);
        if (stdin.isTTY) stdin.setRawMode(false);
        resolve(value);
        return;
      }

      if (key.ctrl && key.name === "c") {
        stdout.write("\n");
        stdin.off("keypress", onKeypress);
        if (stdin.isTTY) stdin.setRawMode(false);
        process.exit(130);
      }

      if (key.name === "backspace") {
        value = value.slice(0, -1);
        return;
      }

      if (char) value += char;
    };

    stdin.on("keypress", onKeypress);
  });
}

function parseStatusFilter(input = "unresolved"): string[] {
  const statuses = input.split(",").map((value) => value.trim().toLowerCase()).filter(Boolean);
  const invalid = statuses.filter((status) => status !== "unresolved" && status !== "all" && !VALID_TICKET_STATUSES.includes(status));

  if (invalid.length > 0) {
    throw new CliError("invalid_args", `Invalid status value(s): ${invalid.join(", ")}`, { input });
  }

  if (statuses.includes("all")) return [];
  if (statuses.includes("unresolved")) return ["new", "open", "pending", "hold"];
  return statuses;
}

function parseLimit(input = "20"): number {
  const limit = Number.parseInt(input, 10);
  if (!Number.isFinite(limit) || Number.isNaN(limit)) {
    throw new CliError("invalid_args", "limit must be an integer", { limit: input });
  }
  if (limit < 1 || limit > 100) {
    throw new CliError("invalid_args", "limit must be between 1 and 100", { limit });
  }
  return limit;
}

function parseSort(input = "desc"): "asc" | "desc" {
  if (input !== "asc" && input !== "desc") {
    throw new CliError("invalid_args", "sort must be asc or desc", { sort: input });
  }
  return input;
}

function buildSearchQuery(base: string, rawStatus: string, statusFilter: string[]): string {
  if (rawStatus === "unresolved") return `${base} status<solved`;
  if (statusFilter.length === 1) return `${base} status:${statusFilter[0]}`;
  return base;
}

function filterStatuses(tickets: SearchTicket[], rawStatus: string, statusFilter: string[]): SearchTicket[] {
  if (rawStatus === "unresolved" || statusFilter.length <= 1) return tickets;
  return tickets.filter((ticket) => statusFilter.includes(String(ticket.status || "").toLowerCase()));
}

async function findUserByEmail(email: string): Promise<User> {
  const data = await apiGet<{ users?: User[] }>("/api/v2/users/search.json", { query: email });
  const users = data.users || [];
  const exact = users.find((user) => user.email?.toLowerCase() === email.toLowerCase());
  if (exact) return exact;
  if (users[0]) return users[0];
  throw new CliError("user_not_found", `No Zendesk user found for email: ${email}`, { email });
}

async function fetchFollowerTickets(email: string, rawStatus: string, limit: number, sort: "asc" | "desc"): Promise<SearchTicket[]> {
  const user = await findUserByEmail(email);
  const statusFilter = parseStatusFilter(rawStatus);
  const baseUrl = new URL(`${apiBaseUrl()}/api/v2/search.json`);
  baseUrl.searchParams.set("query", buildSearchQuery("type:ticket", rawStatus, statusFilter));
  baseUrl.searchParams.set("sort_by", "updated_at");
  baseUrl.searchParams.set("sort_order", sort);
  baseUrl.searchParams.set("per_page", "100");

  const matches: SearchTicket[] = [];
  let nextUrl: string | null = baseUrl.toString();

  while (nextUrl && matches.length < limit) {
    const data = await apiGetUrl<{ results?: SearchTicket[]; next_page?: string | null }>(nextUrl);
    const tickets = filterStatuses(data.results || [], rawStatus, statusFilter);

    for (const ticket of tickets) {
      if ((ticket.follower_ids || []).includes(user.id) && ticket.assignee_id !== user.id) {
        matches.push(ticket);
      }
      if (matches.length >= limit) break;
    }

    nextUrl = data.next_page || null;
  }

  return matches;
}

function apiBaseUrl(): string {
  const { subdomain } = getConfig();
  return `https://${subdomain}.zendesk.com`;
}

program
  .command("configure")
  .description("Set up Zendesk credentials interactively")
  .action(run(async () => {
    const existing = loadConfig();
    process.stderr.write("Zendesk CLI Configuration\n");
    process.stderr.write(`${"─".repeat(30)}\n`);

    const subdomain = await prompt("Subdomain (xxx.zendesk.com)", existing.subdomain);
    const email = await prompt("Email", existing.email);
    const tokenInput = await promptHidden("API Token", Boolean(existing.api_token));
    const api_token = tokenInput || existing.api_token || "";

    saveConfig({ subdomain, email, api_token });
    printJson({ ok: true });
  }));

const TICKET_SUMMARY_FIELDS = [
  "id", "subject", "description", "status", "priority",
  "created_at", "updated_at", "tags",
  "requester_id", "assignee_id", "collaborator_ids", "follower_ids",
  "organization_id", "group_id", "type", "via", "url",
];

function pickTicketFields(ticket: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const key of TICKET_SUMMARY_FIELDS) {
    if (key in ticket) result[key] = ticket[key];
  }
  return result;
}

program
  .command("ticket <id>")
  .description("Get a single ticket")
  .option("--raw", "Output full API response without field filtering")
  .action(run(async (id: string, opts: { raw?: boolean }) => {
    if (!DIGITS_RE.test(id)) {
      throw new CliError("invalid_args", "ticket id must be numeric", { id });
    }

    try {
      const data = await apiGet<{ ticket?: Record<string, unknown> }>(`/api/v2/tickets/${id}.json`);
      if (!data.ticket) {
        throw new CliError("not_found", `Ticket ${id} not found`, { id: Number(id) });
      }
      printJson(opts.raw ? data.ticket : pickTicketFields(data.ticket as Record<string, unknown>));
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) {
        fail("not_found", `Ticket ${id} not found`, { id: Number(id) });
      }
      throw error;
    }
  }));

program
  .command("email <email>")
  .description("Find tickets for an assignee email")
  .option("--status <status>", "unresolved|all|new|open|pending|hold|solved|closed or comma-separated list", "unresolved")
  .option("--limit <n>", "Max tickets to return", "20")
  .option("--sort <order>", "Sort order (asc|desc)", "desc")
  .action(run(async (email: string, opts: { status: string; limit: string; sort: string }) => {
    const limit = parseLimit(opts.limit);
    const sort = parseSort(opts.sort);
    const statusFilter = parseStatusFilter(opts.status);
    const query = buildSearchQuery(`type:ticket assignee:${email}`, opts.status, statusFilter);
    const data = await apiGet<{ results?: SearchTicket[] }>("/api/v2/search.json", {
      query,
      sort_by: "updated_at",
      sort_order: sort,
      per_page: 100,
    });
    const tickets = filterStatuses(data.results || [], opts.status, statusFilter).slice(0, limit);
    printJson(tickets);
  }));

program
  .command("follower <email>")
  .description("Find tickets where the user is a follower but not the assignee")
  .option("--status <status>", "unresolved|all|new|open|pending|hold|solved|closed or comma-separated list", "unresolved")
  .option("--limit <n>", "Max tickets to return", "20")
  .option("--sort <order>", "Sort order (asc|desc)", "desc")
  .action(run(async (email: string, opts: { status: string; limit: string; sort: string }) => {
    const limit = parseLimit(opts.limit);
    const sort = parseSort(opts.sort);
    const tickets = await fetchFollowerTickets(email, opts.status, limit, sort);
    printJson(tickets);
  }));

program
  .command("comments <ticketId>")
  .description("List comments for a ticket")
  .option("--type <type>", "all|public|internal", "all")
  .option("--sort <order>", "Sort order (asc|desc)", "asc")
  .action(run(async (ticketId: string, opts: { type: string; sort: string }) => {
    if (!DIGITS_RE.test(ticketId)) {
      throw new CliError("invalid_args", "ticket id must be numeric", { ticketId });
    }
    const sort = parseSort(opts.sort);
    if (!["all", "public", "internal"].includes(opts.type)) {
      throw new CliError("invalid_args", "type must be all, public, or internal", { type: opts.type });
    }

    const data = await apiGet<{ comments?: Array<Record<string, unknown> & { public?: boolean }> }>(
      `/api/v2/tickets/${ticketId}/comments.json`,
      { sort_order: sort, per_page: 100 }
    );

    let comments = data.comments || [];
    if (opts.type === "public") comments = comments.filter((comment) => comment.public === true);
    if (opts.type === "internal") comments = comments.filter((comment) => comment.public === false);
    printJson(comments);
  }));

function preprocessArgv(argv: string[]): string[] {
  const firstArg = argv[2];
  if (!firstArg || firstArg.startsWith("-") || KNOWN_COMMANDS.has(firstArg)) return argv;
  if (EMAIL_RE.test(firstArg)) return [...argv.slice(0, 2), "email", ...argv.slice(2)];
  if (DIGITS_RE.test(firstArg)) return [...argv.slice(0, 2), "ticket", ...argv.slice(2)];
  return argv;
}

process.argv = preprocessArgv(process.argv);
program.parse();
