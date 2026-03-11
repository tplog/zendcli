/**
 * Config management for zendcli.
 * Credentials are stored in ~/.zendcli/config.json.
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { homedir } from "os";
import { join } from "path";

const CONFIG_DIR = join(homedir(), ".zendcli");
const CONFIG_FILE = join(CONFIG_DIR, "config.json");

export interface ZendConfig {
  subdomain: string;
  email: string;
  api_token: string;
}

export function loadConfig(): Partial<ZendConfig> {
  if (existsSync(CONFIG_FILE)) {
    return JSON.parse(readFileSync(CONFIG_FILE, "utf-8"));
  }
  return {};
}

export function saveConfig(config: ZendConfig): void {
  mkdirSync(CONFIG_DIR, { recursive: true });
  writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2));
}

/** Load and validate config. Throws if not configured. */
export function getConfig(): ZendConfig {
  const config = loadConfig();
  if (!config.subdomain || !config.email || !config.api_token) {
    console.error("Not configured. Run: zend configure");
    process.exit(1);
  }
  return config as ZendConfig;
}
