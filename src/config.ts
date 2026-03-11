/**
 * Config management for zendcli.
 * Credentials are stored in ~/.zendcli/config.json.
 */

import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { homedir } from "os";
import { join } from "path";

const CONFIG_DIR = join(homedir(), ".zendcli");
const CONFIG_FILE = join(CONFIG_DIR, "config.json");

export interface ZendConfig {
  subdomain: string;
  email: string;
  api_token: string;
}

function getEnvConfig(): Partial<ZendConfig> {
  return {
    subdomain: process.env.ZENDESK_SUBDOMAIN,
    email: process.env.ZENDESK_EMAIL,
    api_token: process.env.ZENDESK_API_TOKEN,
  };
}

export function loadConfig(): Partial<ZendConfig> {
  const envConfig = getEnvConfig();
  let fileConfig: Partial<ZendConfig> = {};

  if (existsSync(CONFIG_FILE)) {
    fileConfig = JSON.parse(readFileSync(CONFIG_FILE, "utf-8"));
  }

  return {
    ...fileConfig,
    ...Object.fromEntries(Object.entries(envConfig).filter(([, value]) => value)),
  };
}

export function saveConfig(config: ZendConfig): void {
  mkdirSync(CONFIG_DIR, { recursive: true, mode: 0o700 });
  chmodSync(CONFIG_DIR, 0o700);
  writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2), { mode: 0o600 });
  chmodSync(CONFIG_FILE, 0o600);
}

/** Load and validate config. Throws if not configured. */
export function getConfig(): ZendConfig {
  const config = loadConfig();
  if (!config.subdomain || !config.email || !config.api_token) {
    console.error("Not configured. Run: zend configure or set ZENDESK_SUBDOMAIN, ZENDESK_EMAIL, ZENDESK_API_TOKEN");
    process.exit(1);
  }
  return config as ZendConfig;
}
