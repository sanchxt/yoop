#!/usr/bin/env node

/**
 * Sync npm package versions with Cargo.toml
 *
 * This script reads the version from the root Cargo.toml and updates
 * all package.json files in the npm/ directory to match.
 *
 * Usage:
 *   node scripts/sync-versions.js
 *   node scripts/sync-versions.js --check  # Check only, don't modify
 */

const fs = require("fs");
const path = require("path");

const ROOT_DIR = path.join(__dirname, "..");
const NPM_DIR = path.join(ROOT_DIR, "npm");
const CARGO_TOML = path.join(ROOT_DIR, "Cargo.toml");

function getCargoVersion() {
    const content = fs.readFileSync(CARGO_TOML, "utf8");

    const workspaceMatch = content.match(
        /\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/
    );
    if (workspaceMatch) {
        return workspaceMatch[1];
    }

    const topMatch = content.match(/^version\s*=\s*"([^"]+)"/m);
    if (topMatch) {
        return topMatch[1];
    }

    throw new Error("Could not find version in Cargo.toml");
}

function findPackageJsonFiles() {
    const files = [];

    function walk(dir) {
        const entries = fs.readdirSync(dir, { withFileTypes: true });
        for (const entry of entries) {
            const fullPath = path.join(dir, entry.name);
            if (entry.isDirectory()) {
                if (entry.name !== "node_modules" && entry.name !== "bin") {
                    walk(fullPath);
                }
            } else if (entry.name === "package.json") {
                files.push(fullPath);
            }
        }
    }

    if (fs.existsSync(NPM_DIR)) {
        walk(NPM_DIR);
    }

    return files;
}

function updatePackageJson(filePath, newVersion, checkOnly) {
    const content = fs.readFileSync(filePath, "utf8");
    const pkg = JSON.parse(content);
    const oldVersion = pkg.version;
    let changed = false;

    if (pkg.version !== newVersion) {
        pkg.version = newVersion;
        changed = true;
    }

    if (pkg.optionalDependencies) {
        for (const dep of Object.keys(pkg.optionalDependencies)) {
            if (dep.startsWith("@sanchxt/yoop-")) {
                if (pkg.optionalDependencies[dep] !== newVersion) {
                    pkg.optionalDependencies[dep] = newVersion;
                    changed = true;
                }
            }
        }
    }

    if (changed) {
        const relativePath = path.relative(ROOT_DIR, filePath);
        if (checkOnly) {
            console.log(
                `  ${relativePath}: ${oldVersion} -> ${newVersion} (needs update)`
            );
        } else {
            fs.writeFileSync(filePath, JSON.stringify(pkg, null, 2) + "\n");
            console.log(`  ${relativePath}: ${oldVersion} -> ${newVersion}`);
        }
    }

    return changed;
}

function main() {
    const args = process.argv.slice(2);
    const checkOnly = args.includes("--check");

    console.log("Syncing npm package versions with Cargo.toml...\n");

    const version = getCargoVersion();
    console.log(`Cargo.toml version: ${version}\n`);

    const packageFiles = findPackageJsonFiles();
    if (packageFiles.length === 0) {
        console.log("No package.json files found in npm/ directory.");
        process.exit(0);
    }

    console.log(
        checkOnly
            ? "Checking package.json files:"
            : "Updating package.json files:"
    );

    let updatedCount = 0;
    for (const file of packageFiles) {
        if (updatePackageJson(file, version, checkOnly)) {
            updatedCount++;
        }
    }

    console.log("");

    if (updatedCount === 0) {
        console.log("All package.json files are already up to date.");
    } else if (checkOnly) {
        console.log(`${updatedCount} file(s) need updating.`);
        process.exit(1);
    } else {
        console.log(`Updated ${updatedCount} file(s).`);
    }
}

main();
