/**
 * Zendesk API client.
 * Uses token auth: {email}/token as username, api_token as password.
 */

import { getConfig } from "./config";

function getAuthHeader(): string {
  const { email, api_token } = getConfig();
  const credentials = btoa(`${email}/token:${api_token}`);
  return `Basic ${credentials}`;
}

async function fetchJson<T>(url: string): Promise<T> {
  const resp = await fetch(url, {
    headers: { Authorization: getAuthHeader() },
  });

  if (!resp.ok) {
    const body = await resp.text();
    console.error(`Error ${resp.status}: ${body}`);
    process.exit(1);
  }

  return resp.json();
}

/** Make an authenticated GET request to the Zendesk API. */
export async function apiGet<T = any>(
  path: string,
  params?: Record<string, string | number>
): Promise<T> {
  const { subdomain } = getConfig();
  const url = new URL(`https://${subdomain}.zendesk.com${path}`);

  if (params) {
    for (const [key, value] of Object.entries(params)) {
      url.searchParams.set(key, String(value));
    }
  }

  return fetchJson<T>(url.toString());
}

/** Make an authenticated GET request to a full Zendesk URL. */
export async function apiGetUrl<T = any>(url: string): Promise<T> {
  return fetchJson<T>(url);
}
