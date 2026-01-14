#!/usr/bin/env node

const { execFileSync } = require("child_process");
const { existsSync } = require("fs");
const { join } = require("path");

const PLATFORMS = {
  "linux-x64": "@topo-ui/linux-x64",
  "darwin-arm64": "@topo-ui/darwin-arm64",
  "darwin-x64": "@topo-ui/darwin-x64",
  "win32-x64": "@topo-ui/win32-x64",
};

function getPlatformPackage() {
  const platform = process.platform;
  const arch = process.arch;
  const key = `${platform}-${arch}`;

  const pkg = PLATFORMS[key];
  if (!pkg) {
    console.error(`Unsupported platform: ${platform}-${arch}`);
    console.error(`Supported platforms: ${Object.keys(PLATFORMS).join(", ")}`);
    process.exit(1);
  }

  return pkg;
}

function getBinaryPath() {
  const pkg = getPlatformPackage();

  // Try to find the binary in node_modules
  const binName = process.platform === "win32" ? "topo.exe" : "topo";

  // Check in the package's bin directory
  try {
    const pkgPath = require.resolve(`${pkg}/bin/${binName}`);
    if (existsSync(pkgPath)) {
      return pkgPath;
    }
  } catch (e) {
    // Package not found
  }

  // Fallback: check in parent node_modules
  const paths = [
    join(__dirname, "..", "node_modules", pkg, "bin", binName),
    join(__dirname, "..", "..", pkg, "bin", binName),
    join(__dirname, "..", "..", "..", pkg, "bin", binName),
  ];

  for (const p of paths) {
    if (existsSync(p)) {
      return p;
    }
  }

  console.error(`Could not find topo binary for ${process.platform}-${process.arch}`);
  console.error(`Please ensure ${pkg} is installed.`);
  console.error(`Try: npm install ${pkg}`);
  process.exit(1);
}

try {
  const binaryPath = getBinaryPath();
  const args = process.argv.slice(2);

  execFileSync(binaryPath, args, {
    stdio: "inherit",
    env: process.env,
  });
} catch (error) {
  if (error.status !== undefined) {
    process.exit(error.status);
  }
  console.error(error.message);
  process.exit(1);
}
