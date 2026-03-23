#!/usr/bin/env node

"use strict";

const { execFileSync } = require("child_process");
const path = require("path");

const PLATFORMS = {
  "darwin arm64": "@agiletec-inc/airis-darwin-arm64",
  "darwin x64": "@agiletec-inc/airis-darwin-x64",
  "linux arm64": "@agiletec-inc/airis-linux-arm64",
  "linux x64": "@agiletec-inc/airis-linux-x64",
  "win32 x64": "@agiletec-inc/airis-win32-x64",
};

function getBinaryPath() {
  const key = `${process.platform} ${process.arch}`;
  const pkg = PLATFORMS[key];

  if (!pkg) {
    const supported = Object.keys(PLATFORMS)
      .map((k) => k.replace(" ", "/"))
      .join(", ");
    console.error(
      `Error: Unsupported platform ${process.platform}/${process.arch}\n` +
        `Supported: ${supported}\n` +
        `You can build from source: cargo install airis-monorepo`
    );
    process.exit(1);
  }

  try {
    const binName = process.platform === "win32" ? "airis.exe" : "airis";
    return path.join(
      path.dirname(require.resolve(`${pkg}/package.json`)),
      "bin",
      binName
    );
  } catch {
    console.error(
      `Error: Could not find package ${pkg}\n` +
        `This usually means the optional dependency was not installed.\n` +
        `Try reinstalling: npm install @agiletec-inc/airis\n` +
        `Or build from source: cargo install airis-monorepo`
    );
    process.exit(1);
  }
}

const binary = getBinaryPath();

try {
  const result = execFileSync(binary, process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
  });
} catch (err) {
  if (err.status !== null) {
    process.exit(err.status);
  }
  throw err;
}
