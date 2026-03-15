#!/usr/bin/env node

const { execFileSync } = require("child_process");
const os = require("os");
const path = require("path");

const PLATFORMS = {
  "darwin-arm64": "@tplog/zendcli-darwin-arm64",
  "darwin-x64": "@tplog/zendcli-darwin-x64",
  "linux-x64": "@tplog/zendcli-linux-x64",
  "linux-arm64": "@tplog/zendcli-linux-arm64",
};

const platform = os.platform();
const arch = os.arch();
const key = `${platform}-${arch}`;
const pkg = PLATFORMS[key];

if (!pkg) {
  console.error(`Unsupported platform: ${key}`);
  console.error(`Supported: ${Object.keys(PLATFORMS).join(", ")}`);
  process.exit(1);
}

let binPath;
try {
  binPath = path.join(require.resolve(`${pkg}/package.json`), "..", "zend");
} catch {
  console.error(`Could not find package ${pkg}. Try reinstalling @tplog/zendcli.`);
  process.exit(1);
}

try {
  const result = execFileSync(binPath, process.argv.slice(2), {
    stdio: "inherit",
  });
} catch (e) {
  process.exit(e.status || 1);
}
