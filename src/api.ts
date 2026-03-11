import { getConfig } from "./config";

export class ApiError extends Error {
  status?: number;
  body?: string;
  constructor(message: string, status?: number, body?: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.body = body;
  }
}

function authHeader(): string {
  const { email, api_token } = getConfig();
  const credentials = btoa(`${email}/token:${api_token}`);
  return `Basic ${credentials}`;
}

async function fetchJson<T>(url: string): Promise<T> {
  let resp: Response;

  try {
    resp = await fetch(url, {
      headers: { Authorization: authHeader() },
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Network request failed";
    throw new ApiError(message);
  }

  if (!resp.ok) {
    const body = await resp.text();
    throw new ApiError(`HTTP ${resp.status}`, resp.status, body);
  }

  return resp.json() as Promise<T>;
}

export async function apiGet<T = unknown>(
  path: string,
  params?: Record<string, string | number>
): Promise<T> {
  const { subdomain } = getConfig();
  const url = new URL(`https://${subdomain}.zendesk.com${path}`);

  for (const [key, value] of Object.entries(params || {})) {
    url.searchParams.set(key, String(value));
  }

  return fetchJson<T>(url.toString());
}

export async function apiGetUrl<T = unknown>(url: string): Promise<T> {
  return fetchJson<T>(url);
}
