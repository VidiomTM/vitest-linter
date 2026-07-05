const { execFileSync, execSync } = require("node:child_process");
const fs = require("node:fs");
const https = require("node:https");
const path = require("node:path");
const os = require("node:os");

const REPO = "VidiomTM/vitest-linter";
const BIN_NAME = "vitest-linter";

function getPackageVersion() {
  const pkg = JSON.parse(
    fs.readFileSync(path.join(__dirname, "package.json"), "utf8"),
  );
  return pkg.version;
}

function getTarget() {
  const platform = os.platform();
  const arch = os.arch();

  const platformMap = {
    darwin: "apple-darwin",
    linux: "unknown-linux-gnu",
    win32: "pc-windows-msvc",
  };
  const archMap = { x64: "x86_64", arm64: "aarch64" };

  if (!platformMap[platform] || !archMap[arch]) {
    console.error(`Unsupported platform: ${platform}-${arch}`);
    process.exit(1);
  }

  return `${archMap[arch]}-${platformMap[platform]}`;
}

function getArtifactUrl(version, target) {
  const ext = os.platform() === "win32" ? ".zip" : ".tar.gz";
  const name = `${BIN_NAME}-${target}${ext}`;
  return `https://github.com/${REPO}/releases/download/v${version}/${name}`;
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const request = (u, redirects) => {
      if (redirects > 5) return reject(new Error("Too many redirects"));
      https
        .get(u, { headers: { "User-Agent": "node" } }, (res) => {
          if (
            res.statusCode >= 300 &&
            res.statusCode < 400 &&
            res.headers.location
          ) {
            return request(res.headers.location, redirects + 1);
          }
          if (res.statusCode !== 200) {
            return reject(new Error(`HTTP ${res.statusCode} downloading ${u}`));
          }
          const stream = fs.createWriteStream(dest);
          res.pipe(stream);
          stream.on("finish", () => {
            stream.close();
            resolve();
          });
          stream.on("error", reject);
        })
        .on("error", reject);
    };
    request(url, 0);
  });
}

function extract(archive, destDir) {
  if (os.platform() === "win32") {
    execFileSync(
      "powershell",
      [
        "-Command",
        "Expand-Archive",
        "-Path",
        archive,
        "-DestinationPath",
        destDir,
        "-Force",
      ],
      { stdio: "inherit" },
    );
  } else {
    execFileSync("tar", ["-xzf", archive, "-C", destDir], { stdio: "inherit" });
  }
}

async function main() {
  const version = getPackageVersion();
  const target = getTarget();
  const binDir = path.join(__dirname, "bin");
  const ext = os.platform() === "win32" ? ".exe" : "";
  const binPath = path.join(binDir, `${BIN_NAME}-bin${ext}`);

  fs.mkdirSync(binDir, { recursive: true });

  if (fs.existsSync(binPath)) {
    return;
  }

  const url = getArtifactUrl(version, target);
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "vitest-linter-"));
  const archiveExt = os.platform() === "win32" ? ".zip" : ".tar.gz";
  const archivePath = path.join(tmpDir, `dist${archiveExt}`);

  try {
    console.log(`Downloading vitest-linter v${version} for ${target}...`);
    await download(url, archivePath);
    extract(archivePath, tmpDir);

    const extracted = path.join(tmpDir, `${BIN_NAME}${ext}`);
    if (!fs.existsSync(extracted)) {
      const entries = fs.readdirSync(tmpDir);
      const subDir = entries.find((e) =>
        fs.statSync(path.join(tmpDir, e)).isDirectory(),
      );
      if (subDir) {
        const inner = path.join(tmpDir, subDir, `${BIN_NAME}${ext}`);
        if (fs.existsSync(inner)) {
          fs.copyFileSync(inner, binPath);
        }
      }
    } else {
      fs.copyFileSync(extracted, binPath);
    }

    if (!fs.existsSync(binPath)) {
      throw new Error(
        `Binary not found after extraction. Looked for ${binPath}`,
      );
    }

    fs.chmodSync(binPath, 0o755);
    console.log(`vitest-linter v${version} installed successfully.`);
  } catch (err) {
    console.warn(`Prebuilt binary download failed: ${err.message}`);
    console.warn("Falling back to cargo install...");
    try {
      execSync("cargo install vitest-linter --locked", { stdio: "inherit" });
      const cargoHome =
        process.env.CARGO_HOME || path.join(os.homedir(), ".cargo");
      const cargoBin =
        os.platform() === "win32"
          ? path.join(cargoHome, "bin", `${BIN_NAME}.exe`)
          : path.join(cargoHome, "bin", BIN_NAME);
      if (fs.existsSync(cargoBin)) {
        fs.copyFileSync(cargoBin, binPath);
        console.log("Installed via cargo install.");
      }
    } catch (cargoErr) {
      console.error(`cargo install also failed: ${cargoErr.message}`);
      console.error(
        "Please install Rust (https://rustup.rs) or download the binary manually.",
      );
      process.exit(1);
    }
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

main();
