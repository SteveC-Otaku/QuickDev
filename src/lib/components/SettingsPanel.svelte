<script lang="ts">
  import type { Settings } from "../types";

  interface Props {
    settings: Settings;
    onchange?: (settings: Settings) => void;
    onexport?: () => void | Promise<void>;
    onimport?: (file: File) => void | Promise<void>;
  }

  let { settings, onchange, onexport, onimport }: Props = $props();

  let importInput: HTMLInputElement | undefined = $state();

  function toggle(key: "autoRefresh" | "minimizeToTray" | "autoStart" | "writeLogs") {
    onchange?.({ ...settings, [key]: !settings[key] });
  }

  function updateHotkey(value: string) {
    onchange?.({ ...settings, hotkey: value });
  }

  function updateShell(value: string) {
    onchange?.({ ...settings, shell: value });
  }

  function removePort(index: number) {
    const next = settings.highlightPorts.filter((_, i) => i !== index);
    onchange?.({ ...settings, highlightPorts: next });
  }

  function addPort() {
    const raw = prompt("输入要高亮的端口号");
    if (!raw?.trim()) return;
    const port = Number(raw.trim());
    if (!port || port < 1 || port > 65535) {
      alert("端口必须是 1–65535 之间的数字");
      return;
    }
    if (settings.highlightPorts.includes(port)) return;
    onchange?.({ ...settings, highlightPorts: [...settings.highlightPorts, port].sort((a, b) => a - b) });
  }

  function removeGroup(index: number) {
    const next = (settings.groups ?? []).filter((_, i) => i !== index);
    onchange?.({ ...settings, groups: next });
  }

  function addGroup() {
    const raw = prompt("输入分组名称（如：工作、个人）");
    if (!raw?.trim()) return;
    const name = raw.trim();
    const list = settings.groups ?? [];
    if (list.includes(name)) return;
    onchange?.({ ...settings, groups: [...list, name] });
  }

  function pickImportFile() {
    importInput?.click();
  }

  async function handleImportFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    await onimport?.(file);
    input.value = "";
  }
</script>

<div class="h-full min-h-0 overflow-y-auto overscroll-contain px-4 py-3">
  <section class="mb-5">
    <h3 class="text-[11px] font-semibold text-qd-muted uppercase tracking-wider mb-2.5">常规</h3>
    {#each [
      { key: "minimizeToTray" as const, label: "关闭窗口时最小化到托盘", desc: "关闭主窗口后继续在托盘运行" },
      { key: "autoRefresh" as const, label: "端口列表自动刷新", desc: "每 5 秒自动刷新监听端口" },
      { key: "autoStart" as const, label: "开机自启", desc: "登录 Windows 后自动启动托盘（不自动运行 dev）" },
      { key: "writeLogs" as const, label: "日志写入本地文件", desc: "%LOCALAPPDATA%\\QuickDev\\logs\\{项目ID}.log" },
    ] as row}
      <div class="flex items-center justify-between px-3 py-2.5 bg-qd-surface border border-qd-border rounded-lg mb-1.5">
        <div>
          <div class="text-[13px] font-medium text-qd-ink">{row.label}</div>
          <div class="text-[11px] text-qd-dim mt-0.5">{row.desc}</div>
        </div>
        <button
          class="w-9 h-5 rounded-full border border-qd-border relative cursor-pointer shrink-0 transition-colors"
          class:bg-qd-brand={settings[row.key]}
          class:border-qd-brand={settings[row.key]}
          class:bg-qd-elevated={!settings[row.key]}
          onclick={() => toggle(row.key)}
          aria-label={row.label}
        >
          <span
            class="absolute w-3.5 h-3.5 rounded-full bg-white top-0.5 left-0.5 transition-transform"
            class:translate-x-4={settings[row.key]}
          ></span>
        </button>
      </div>
    {/each}

    <div class="px-3 py-2.5 bg-qd-surface border border-qd-border rounded-lg mt-1.5">
      <div class="text-[13px] font-medium text-qd-ink mb-1">全局热键唤起面板</div>
      <div class="text-[11px] text-qd-dim mb-2">默认 Ctrl+Shift+D，保存后立即生效</div>
      <input
        class="w-full bg-qd-elevated border border-qd-border rounded-lg px-2.5 py-1.5 font-mono text-xs text-qd-ink outline-none focus:border-qd-brand"
        value={settings.hotkey ?? "Ctrl+Shift+D"}
        onchange={(e) => updateHotkey(e.currentTarget.value)}
        placeholder="Ctrl+Shift+D"
      />
    </div>

    <div class="px-3 py-2.5 bg-qd-surface border border-qd-border rounded-lg mt-1.5">
      <div class="text-[13px] font-medium text-qd-ink mb-1">Shell 路径</div>
      <div class="text-[11px] text-qd-dim mb-2">启动 dev 时使用的 shell，如 powershell 或 cmd</div>
      <input
        class="w-full bg-qd-elevated border border-qd-border rounded-lg px-2.5 py-1.5 font-mono text-xs text-qd-ink outline-none focus:border-qd-brand"
        value={settings.shell ?? "powershell"}
        onchange={(e) => updateShell(e.currentTarget.value)}
        placeholder="powershell"
      />
    </div>
  </section>

  <section class="mb-5">
    <h3 class="text-[11px] font-semibold text-qd-muted uppercase tracking-wider mb-2.5">Dev 端口高亮</h3>
    <div class="flex flex-wrap gap-1.5 px-3 py-2.5 bg-qd-surface border border-qd-border rounded-lg">
      {#each settings.highlightPorts as port, i}
        <button
          type="button"
          class="font-mono text-[11px] font-medium px-2 py-0.5 rounded bg-qd-brand-soft text-qd-brand hover:ring-1 hover:ring-qd-danger hover:bg-qd-danger-soft hover:text-qd-danger"
          title="点击移除"
          onclick={() => removePort(i)}
        >
          {port}
        </button>
      {/each}
      <button
        type="button"
        class="font-mono text-[11px] px-2 py-0.5 rounded text-qd-muted hover:text-qd-brand"
        onclick={addPort}
      >
        + 添加
      </button>
    </div>
  </section>

  <section class="mb-5">
    <h3 class="text-[11px] font-semibold text-qd-muted uppercase tracking-wider mb-2.5">项目分组</h3>
    <div class="flex flex-wrap gap-1.5 px-3 py-2.5 bg-qd-surface border border-qd-border rounded-lg">
      {#each settings.groups ?? [] as group, i}
        <button
          type="button"
          class="text-[11px] font-medium px-2 py-0.5 rounded bg-qd-elevated text-qd-body border border-qd-border hover:ring-1 hover:ring-qd-danger hover:text-qd-danger"
          title="点击移除"
          onclick={() => removeGroup(i)}
        >
          {group}
        </button>
      {/each}
      <button type="button" class="text-[11px] px-2 py-0.5 rounded text-qd-muted hover:text-qd-brand" onclick={addGroup}>
        + 添加
      </button>
    </div>
    <p class="text-[11px] text-qd-dim mt-2">添加/编辑项目时可从列表选择分组</p>
  </section>

  <section>
    <h3 class="text-[11px] font-semibold text-qd-muted uppercase tracking-wider mb-2.5">配置备份</h3>
    <div class="flex gap-2">
      <button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" onclick={() => onexport?.()}>导出 JSON</button>
      <button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" onclick={pickImportFile}>导入 JSON</button>
      <input bind:this={importInput} type="file" accept=".json,application/json" class="hidden" onchange={handleImportFile} />
    </div>
    <p class="text-[11px] text-qd-dim mt-2">导入会覆盖当前项目与设置</p>
  </section>
</div>
