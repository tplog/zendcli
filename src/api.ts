/**
 * Zendesk API client.
 * Uses token auth: {email}/token as username, api_token as password.
 */

import { getConfig } from "./config";

/** Make an authenticated GET request to the Zendesk API. */
export async function apiGet<T = any>(
  path: string,
  params?: Record<string, string | number>
): Promise<T> {
  const { subdomain, email, api_token } = getConfig();
  const url = new URL(`https://${subdomain}.zendesk.com${path}`);

  if (params) {
    for (const [key, value] of Object.entries(params)) {
      url.searchParams.set(key, String(value));
    }
  }

  // Zendesk token auth: Base64({email}/token:{api_token})
  const credentials = btoa(`${email}/token:${api_token}`);

  const resp = await fetch(url.toString(), {
    headers: { Authorization: `Basic ${credentials}` },
  });

  if (!resp.ok) {
    const body = await resp.text();
    console.error(`Error ${resp.status}: ${body}`);
    process.exit(1);
  }

  return resp.json();
}
