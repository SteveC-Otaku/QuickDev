// Usage: npm run release [-- --skip-build] [-- --upload]
// Env:   QUICKDEV_OSS_BASE   public URL of the site root (default: https://quickdev.steve-lab.cc)
//        QUICKDEV_OSS_TARGET ossutil destination, e.g. oss://quickdev
import { execSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const args = new Set(process.argv.slice(2));
const readJson = (p) => JSON.parse(fs.readFileSync(path.join(root, p), "utf8"));

const version = readJson("src-tauri/tauri.conf.json").version;
const changelog = readJson("release/changelog.json");
if (changelog[0]?.version !== version) throw new Error(`release/changelog.json 顶部缺少 v${version} 的记录`);

const ossBase = (process.env.QUICKDEV_OSS_BASE ?? "https://quickdev.steve-lab.cc").replace(/\/+$/, "");
const ghBase = `https://github.com/SteveC-Otaku/QuickDev/releases/download/v${version}`;
const keyPath = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH ?? path.join(os.homedir(), ".tauri", "quickdev-updater.key");
const targets = [
  { triple: "x86_64-pc-windows-msvc", arch: "x64", platform: "windows-x86_64" },
  { triple: "aarch64-pc-windows-msvc", arch: "arm64", platform: "windows-aarch64" },
];

/* chapter: build installers */
if (!args.has("--skip-build")) {
  const env = { ...process.env, TAURI_SIGNING_PRIVATE_KEY: fs.readFileSync(keyPath, "utf8"), TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "" };
  for (const t of targets) execSync(`npx tauri build --target ${t.triple}`, { cwd: root, stdio: "inherit", env });
}

/* chapter: collect artifacts + update manifests */
const dist = path.join(root, "release", "dist");
const oss = path.join(dist, "oss");
const gh = path.join(dist, "github");
fs.rmSync(dist, { recursive: true, force: true });
fs.mkdirSync(path.join(oss, "downloads"), { recursive: true });
fs.mkdirSync(gh, { recursive: true });

for (const t of targets) {
  const dir = path.join(root, "src-tauri", "target", t.triple, "release", "bundle", "nsis");
  const exe = fs.readdirSync(dir).find((f) => f.endsWith(`_${version}_${t.arch}-setup.exe`));
  if (!exe || !fs.existsSync(path.join(dir, `${exe}.sig`))) throw new Error(`缺少 ${t.arch} 安装包或签名：${dir}`);
  t.name = `QuickDev_${version}_${t.arch}-setup.exe`;
  t.sig = fs.readFileSync(path.join(dir, `${exe}.sig`), "utf8").trim();
  t.size = `${(fs.statSync(path.join(dir, exe)).size / 1048576).toFixed(1)} MB`;
  fs.copyFileSync(path.join(dir, exe), path.join(oss, "downloads", t.name));
  fs.copyFileSync(path.join(dir, exe), path.join(gh, t.name));
}

const manifest = (base) => JSON.stringify({
  version,
  notes: changelog[0].notes.map((n) => `- ${n}`).join("\n"),
  pub_date: new Date().toISOString(),
  platforms: Object.fromEntries(targets.map((t) => [t.platform, { signature: t.sig, url: `${base}/${t.name}` }])),
}, null, 2);
fs.writeFileSync(path.join(gh, "latest.json"), manifest(ghBase));
fs.writeFileSync(path.join(oss, "latest.json"), manifest(`${ossBase}/downloads`));
fs.copyFileSync(path.join(root, "release", "changelog.json"), path.join(oss, "changelog.json"));

/* chapter: render website */
const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const releasesHtml = changelog.map((r) => `        <li class="release">
          <div class="release-h"><b>v${esc(r.version)}</b><span>${esc(r.title)}</span><time datetime="${r.date}">${r.date}</time></div>
          <ul>${r.notes.map((n) => `<li>${esc(n)}</li>`).join("")}</ul>
        </li>`).join("\n");
const html = fs.readFileSync(path.join(root, "website", "index.html"), "utf8")
  .replaceAll("__VERSION__", version)
  .replaceAll("__SIZE_X64__", targets[0].size)
  .replaceAll("__SIZE_ARM64__", targets[1].size)
  .replace("<!--CHANGELOG-->", releasesHtml);
fs.writeFileSync(path.join(oss, "index.html"), html);
const imgSrc = path.join(root, "website", "img");
const imgDst = path.join(oss, "img");
for (const f of fs.readdirSync(imgSrc, { recursive: true })) {
  const from = path.join(imgSrc, f);
  if (!fs.statSync(from).isFile()) continue;
  const to = path.join(imgDst, f);
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.copyFileSync(from, to);
}

console.log(`[release] v${version} → ${path.relative(root, oss)}（上传 OSS）与 ${path.relative(root, gh)}（GitHub Release）`);
for (const t of targets) console.log(`  ${t.name}  ${t.size}`);

/* chapter: upload to OSS (manifest last, so clients never see a version whose installer is missing) */
if (args.has("--upload")) {
  const dest = process.env.QUICKDEV_OSS_TARGET?.replace(/\/+$/, "");
  if (!dest) throw new Error("请设置 QUICKDEV_OSS_TARGET，例如 oss://my-bucket/quickdev");
  const run = (cmd) => execSync(cmd, { cwd: oss, stdio: "inherit" });
  run(`ossutil cp -r -f downloads ${dest}/downloads`);
  run(`ossutil cp -r -f img ${dest}/img`);
  for (const f of ["changelog.json", "index.html", "latest.json"]) run(`ossutil cp -f ${f} ${dest}/${f} --cache-control no-cache`);
}
