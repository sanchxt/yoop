#!/usr/bin/env node

const { spawn } = require("child_process");
const path = require("path");
const fs = require("fs");

const PLATFORMS = {
    "linux-x64": {
        packages: [
            "@sanchxt/yoop-linux-x64-musl",
            "@sanchxt/yoop-linux-x64-gnu",
        ],
    },
    "linux-arm64": {
        packages: ["@sanchxt/yoop-linux-arm64-gnu"],
    },
    "darwin-x64": {
        packages: ["@sanchxt/yoop-darwin-x64"],
    },
    "darwin-arm64": {
        packages: ["@sanchxt/yoop-darwin-arm64"],
    },
    "win32-x64": {
        packages: ["@sanchxt/yoop-win32-x64-msvc"],
    },
};

function getBinaryPath() {
    const platform = process.platform;
    const arch = process.arch;
    const key = `${platform}-${arch}`;

    const platformConfig = PLATFORMS[key];
    if (!platformConfig) {
        console.error(`Error: Unsupported platform: ${platform}-${arch}`);
        console.error(
            "Supported platforms: linux-x64, linux-arm64, darwin-x64, darwin-arm64, win32-x64"
        );
        process.exit(1);
    }

    const binName = platform === "win32" ? "yoop.exe" : "yoop";

    for (const packageName of platformConfig.packages) {
        try {
            const packagePath = require.resolve(`${packageName}/package.json`);
            const binPath = path.join(
                path.dirname(packagePath),
                "bin",
                binName
            );

            if (fs.existsSync(binPath)) {
                return binPath;
            }
        } catch {
            continue;
        }
    }

    console.error(`Error: Could not find yoop binary for ${platform}-${arch}`);
    console.error("");
    console.error(
        "This usually means the platform-specific package failed to install."
    );
    console.error("Try reinstalling:");
    console.error("  npm install -g yoop");
    console.error("");
    console.error("If the problem persists, please file an issue at:");
    console.error("  https://github.com/sanchxt/yoop/issues");
    process.exit(1);
}

function run() {
    const binPath = getBinaryPath();

    const child = spawn(binPath, process.argv.slice(2), {
        stdio: "inherit",
        shell: false,
    });

    child.on("error", (err) => {
        console.error(`Error: Failed to execute yoop: ${err.message}`);
        process.exit(1);
    });

    child.on("exit", (code, signal) => {
        if (signal) {
            process.exit(1);
        }
        process.exit(code ?? 0);
    });
}

run();
