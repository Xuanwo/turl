#!/usr/bin/env node
// Unified entry point for the xurl CLI.

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const require = createRequire(import.meta.url);

const PLATFORM_PACKAGE_BY_TARGET = {
  "x86_64-unknown-linux-gnu": "@xuanwo/xurl-linux-x64",
  "aarch64-unknown-linux-gnu": "@xuanwo/xurl-linux-arm64",
  "x86_64-apple-darwin": "@xuanwo/xurl-darwin-x64",
  "aarch64-apple-darwin": "@xuanwo/xurl-darwin-arm64",
  "x86_64-pc-windows-msvc": "@xuanwo/xurl-win32-x64",
  "aarch64-pc-windows-msvc": "@xuanwo/xurl-win32-arm64",
};

function detectTargetTriple(platformName, archName) {
  switch (platformName) {
    case "linux":
      if (archName === "x64") {
        return "x86_64-unknown-linux-gnu";
      }
      if (archName === "arm64") {
        return "aarch64-unknown-linux-gnu";
      }
      break;
    case "darwin":
      if (archName === "x64") {
        return "x86_64-apple-darwin";
      }
      if (archName === "arm64") {
        return "aarch64-apple-darwin";
      }
      break;
    case "win32":
      if (archName === "x64") {
        return "x86_64-pc-windows-msvc";
      }
      if (archName === "arm64") {
        return "aarch64-pc-windows-msvc";
      }
      break;
    default:
      break;
  }
  return null;
}

function detectPackageManager() {
  const userAgent = process.env.npm_config_user_agent || "";
  if (/\bbun\//.test(userAgent)) {
    return "bun";
  }
  return userAgent ? "npm" : null;
}

const targetTriple = detectTargetTriple(process.platform, process.arch);
if (!targetTriple) {
  throw new Error(`Unsupported platform: ${process.platform} (${process.arch})`);
}

const platformPackage = PLATFORM_PACKAGE_BY_TARGET[targetTriple];
if (!platformPackage) {
  throw new Error(`Unsupported target triple: ${targetTriple}`);
}

const binaryName = process.platform === "win32" ? "xurl.exe" : "xurl";
const localVendorRoot = path.join(__dirname, "..", "vendor");
const localBinaryPath = path.join(localVendorRoot, targetTriple, "xurl", binaryName);

let vendorRoot;
try {
  const packageJsonPath = require.resolve(`${platformPackage}/package.json`);
  vendorRoot = path.join(path.dirname(packageJsonPath), "vendor");
} catch {
  if (existsSync(localBinaryPath)) {
    vendorRoot = localVendorRoot;
  } else {
    const manager = detectPackageManager();
    const updateCommand =
      manager === "bun"
        ? "bun install -g @xuanwo/xurl@latest"
        : "npm install -g @xuanwo/xurl@latest";
    throw new Error(
      `Missing optional dependency ${platformPackage}. Reinstall xurl: ${updateCommand}`,
    );
  }
}

const binaryPath = path.join(vendorRoot, targetTriple, "xurl", binaryName);
const env = { ...process.env };
env[detectPackageManager() === "bun" ? "XURL_MANAGED_BY_BUN" : "XURL_MANAGED_BY_NPM"] =
  "1";

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env,
});

child.on("error", (err) => {
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});

const forwardSignal = (signal) => {
  if (child.killed) {
    return;
  }
  try {
    child.kill(signal);
  } catch {
    // Ignore errors when the child already exited.
  }
};

["SIGINT", "SIGTERM", "SIGHUP"].forEach((signal) => {
  process.on(signal, () => forwardSignal(signal));
});

const childResult = await new Promise((resolve) => {
  child.on("exit", (code, signal) => {
    if (signal) {
      resolve({ type: "signal", signal });
    } else {
      resolve({ type: "code", exitCode: code ?? 1 });
    }
  });
});

if (childResult.type === "signal") {
  process.kill(process.pid, childResult.signal);
} else {
  process.exit(childResult.exitCode);
}
