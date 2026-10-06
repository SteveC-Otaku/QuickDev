import { isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { check, type Update } from "@tauri-apps/plugin-updater";
import changelog from "../../release/changelog.json";

export type ChangelogEntry = { version: string; date: string; title: string; notes: string[] };
export const releases = changelog as ChangelogEntry[];

class Updater {
  current = $state("");
  checking = $state(false);
  available = $state<Update | null>(null);
  status = $state("");
  dialogOpen = $state(false);
  progress = $state<{ done: number; total: number } | null>(null);
  error = $state("");

  async init() {
    if (!isTauri()) return;
    this.current = await getVersion();
    setTimeout(() => void this.check(true), 8000);
  }

  async check(silent = false) {
    if (this.checking || this.progress) return;
    this.checking = true;
    this.error = "";
    if (!silent) this.status = "正在检查更新…";
    try {
      this.available = await check({ timeout: 15000 });
      this.status = this.available ? `发现新版本 v${this.available.version}` : `已是最新版本 v${this.current}`;
      if (this.available) this.dialogOpen = true;
    } catch (e) {
      this.status = silent ? "" : `检查更新失败：${String(e)}`;
    } finally {
      this.checking = false;
    }
  }

  async install() {
    const update = this.available;
    if (!update || this.progress) return;
    this.error = "";
    this.progress = { done: 0, total: 0 };
    try {
      await update.downloadAndInstall((e) => {
        if (e.event === "Started") this.progress = { done: 0, total: e.data.contentLength ?? 0 };
        else if (e.event === "Progress" && this.progress) this.progress = { ...this.progress, done: this.progress.done + e.data.chunkLength };
      });
    } catch (e) {
      this.error = `更新失败：${String(e)}`;
      this.progress = null;
    }
  }
}

export const updater = new Updater();
