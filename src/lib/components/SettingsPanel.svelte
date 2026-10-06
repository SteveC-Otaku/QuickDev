<script lang="ts">
  import ActionIcon from "./ActionIcon.svelte";
  import type { Settings } from "../types";
  import { updater, releases } from "../update.svelte";

  interface Props {
    settings: Settings;
    busy?: boolean;
    configBusy?: boolean;
    onchange?: (settings: Settings) => void;
    onexport?: () => void | Promise<void>;
    onimport?: (file: File) => void | Promise<void>;
    onopendirectory?: (kind: 'config' | 'logs') => void;
  }

  let { settings, busy = false, configBusy = false, onchange, onexport, onimport, onopendirectory }: Props = $props();

  let importInput: HTMLInputElement | undefined = $state();

  let newPort = $state("");
  let newGroup = $state("");
  let error = $state("");

  function patch(partial: Partial<Settings>) {
    if (!busy) onchange?.({ ...settings, ...partial });
  }

  function toggle(key: "autoRefresh" | "minimizeToTray" | "autoStart" | "writeLogs" | "trayAnimation") {
    patch({ [key]: !settings[key] });
  }

  function removePort(index: number) {
    patch({ highlightPorts: settings.highlightPorts.filter((_, i) => i !== index) });
  }

  function addPort() {
    error = "";
    const port = Number(newPort.trim());
    if (!newPort.trim() || !Number.isInteger(port) || port < 1 || port > 65535) {
      error = "端口必须是 1–65535 之间的整数"; return;
    }
    if (settings.highlightPorts.includes(port)) { error = "该端口已在列表中"; return; }
    patch({ highlightPorts: [...settings.highlightPorts, port].sort((a, b) => a - b) });
    newPort = "";
  }

  function removeGroup(index: number) {
    patch({ groups: (settings.groups ?? []).filter((_, i) => i !== index) });
  }

  function addGroup() {
    error = "";
    const name = newGroup.trim();
    if (!name) { error = "请填写分组名称"; return; }
    const list = settings.groups ?? [];
    if (list.includes(name)) { error = "该分组已存在"; return; }
    patch({ groups: [...list, name] });
    newGroup = "";
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
  <div class="qd-save-status" role="status">{#if busy || configBusy}<span class="qd-spinner" aria-hidden="true"></span>{busy ? "正在保存设置…" : "正在处理配置…"}{:else}设置修改后自动保存{/if}</div>
  <fieldset disabled={busy || configBusy} class="contents">
  <section class="mb-5">
    <h3 class="text-[11px] font-semibold text-qd-muted uppercase tracking-wider mb-2.5">常规</h3>
    {#each [
      { key: "minimizeToTray" as const, label: "关闭窗口时最小化到托盘", desc: "关闭主窗口后继续在托盘运行" },
      { key: "autoRefresh" as const, label: "端口列表自动刷新", desc: `每 ${settings.refreshIntervalSecs ?? 5} 秒刷新；只在端口页扫描端口` },
      { key: "autoStart" as const, label: "开机自启", desc: "登录 Windows 后自动启动托盘（不自动运行 dev）" },
      { key: "trayAnimation" as const, label: "托盘仪表盘动画", desc: "运行时摆针；系统关闭动画时使用静态图标" },
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
          role="switch"
          aria-checked={!!settings[row.key]}
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
      <div class="text-[13px] font-medium text-qd-ink mb-2">自动刷新间隔</div>
      <div class="flex flex-wrap gap-2" role="group" aria-label="自动刷新间隔">
        {#each [3,5,10,30] as secs}
          <button class="qd-btn qd-btn-secondary qd-btn-sm" aria-pressed={(settings.refreshIntervalSecs ?? 5) === secs} disabled={!settings.autoRefresh} onclick={() => patch({refreshIntervalSecs:secs})}>{secs} 秒</button>
        {/each}
      </div>
      <p class="text-[11px] text-qd-dim mt-2">电脑负载较高时可选 10 或 30 秒，手动刷新仍可随时使用。</p>
    </div>
    <div class="px-3 py-2.5 bg-qd-surface border border-qd-border rounded-lg mt-1.5">
      <div class="text-[13px] font-medium text-qd-ink mb-2">打开即栈时的默认页面</div>
      <div class="flex gap-2" role="group" aria-label="默认首页">
        {#each [{id:'ports',label:'端口'},{id:'projects',label:'项目'},{id:'settings',label:'设置'}] as page}
          <button class="qd-btn qd-btn-secondary qd-btn-sm" aria-pressed={(settings.startPage ?? 'ports') === page.id} onclick={() => patch({startPage:page.id as Settings['startPage']})}>{page.label}</button>
        {/each}
      </div>
      <p class="text-[11px] text-qd-dim mt-2">下次启动生效，不改变当前页面。</p>
    </div>

    <div class="px-3 py-2.5 bg-qd-surface border border-qd-border rounded-lg mt-1.5">
      <div class="text-[13px] font-medium text-qd-ink mb-1">全局热键唤起面板（选填）</div>
      <div class="text-[11px] text-qd-dim mb-2">默认 Ctrl+Shift+D；留空关闭热键，保存后立即生效。</div>
      <input
        class="w-full bg-qd-elevated border border-qd-border rounded-lg px-2.5 py-1.5 font-mono text-xs text-qd-ink outline-none focus:border-qd-brand"
        aria-label="全局热键"
        value={settings.hotkey ?? "Ctrl+Shift+D"}
        onchange={(e) => patch({ hotkey: e.currentTarget.value.trim() })}
        placeholder="留空关闭热键"
      />
    </div>

    <div class="px-3 py-2.5 bg-qd-surface border border-qd-border rounded-lg mt-1.5">
      <div class="text-[13px] font-medium text-qd-ink mb-1">Shell 路径</div>
      <div class="text-[11px] text-qd-dim mb-2">启动 dev 时使用的 shell，如 powershell 或 cmd</div>
      <input
        class="w-full bg-qd-elevated border border-qd-border rounded-lg px-2.5 py-1.5 font-mono text-xs text-qd-ink outline-none focus:border-qd-brand"
        aria-label="Shell 路径"
        value={settings.shell ?? "powershell"}
        onchange={(e) => patch({ shell: e.currentTarget.value })}
        placeholder="powershell"
      />
      <div class="flex gap-2 mt-2" role="group" aria-label="Shell 预设">
        {#each [{value:'powershell',label:'PowerShell'},{value:'cmd',label:'CMD'}] as preset}
          <button class="qd-btn qd-btn-secondary qd-btn-sm" aria-pressed={(settings.shell ?? 'powershell').replace(/\.exe$/i,'').toLowerCase() === preset.value} onclick={() => patch({shell:preset.value})}>{preset.label}</button>
        {/each}
      </div>
      <p class="text-[11px] text-qd-dim mt-2">CMD 适合 npm / pnpm 等命令，省去 PowerShell 初始化；包含 PowerShell 语法的命令请保留 PowerShell。</p>
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
      <input class="qd-input qd-input-short" bind:value={newPort} aria-label="新增高亮端口" inputmode="numeric" placeholder="端口号" onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); addPort(); } }} />
      <button
        type="button"
        class="font-mono text-[11px] px-2 py-0.5 rounded text-qd-muted hover:text-qd-brand"
        onclick={addPort}
      >
        <ActionIcon name="plus" size={14} /> 添加
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
      <input class="qd-input qd-input-short" bind:value={newGroup} aria-label="新增分组" placeholder="分组名称" onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); addGroup(); } }} />
      <button type="button" class="text-[11px] px-2 py-0.5 rounded text-qd-muted hover:text-qd-brand" onclick={addGroup}>
        <ActionIcon name="plus" size={14} /> 添加
      </button>
    </div>
    <p class="text-[11px] text-qd-dim mt-2">添加/编辑项目时可从列表选择分组</p>
  </section>

  <section>
    <h3 class="text-[11px] font-semibold text-qd-muted uppercase tracking-wider mb-2.5">配置备份</h3>
    <div class="flex gap-2">
      <button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" onclick={() => onexport?.()}><ActionIcon name="download" size={14} /> 导出 JSON</button>
      <button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" onclick={pickImportFile}><ActionIcon name="upload" size={14} /> 导入 JSON</button>
      <input bind:this={importInput} type="file" accept=".json,application/json" class="hidden" onchange={handleImportFile} />
    </div>
    <p class="text-[11px] text-qd-dim mt-2">导入会覆盖当前项目与设置</p>
    <div class="flex gap-2 mt-3">
      <button class="qd-btn qd-btn-secondary qd-btn-sm" onclick={() => onopendirectory?.('logs')}><ActionIcon name="folder" size={14} /> 打开日志目录</button>
      <button class="qd-btn qd-btn-secondary qd-btn-sm" onclick={() => onopendirectory?.('config')}><ActionIcon name="folder" size={14} /> 打开配置目录</button>
    </div>
  </section>
  </fieldset>

  <section class="mt-5">
    <h3 class="text-[11px] font-semibold text-qd-muted uppercase tracking-wider mb-2.5">关于与更新</h3>
    <div class="flex items-center justify-between gap-3 px-3 py-2.5 bg-qd-surface border border-qd-border rounded-lg">
      <div>
        <div class="text-[13px] font-medium text-qd-ink">即栈 Quick Dev {updater.current ? `v${updater.current}` : ""}</div>
        <div class="text-[11px] text-qd-dim mt-0.5" role="status">{updater.status || "启动时自动检查更新，仅此时联网"}</div>
      </div>
      {#if updater.available}
        <button class="qd-btn qd-btn-primary qd-btn-sm shrink-0" onclick={() => updater.dialogOpen = true}>更新到 v{updater.available.version}</button>
      {:else}
        <button class="qd-btn qd-btn-secondary qd-btn-sm shrink-0" disabled={updater.checking || !updater.current} onclick={() => updater.check()}>
          {#if updater.checking}<span class="qd-spinner" aria-hidden="true"></span>{:else}<ActionIcon name="refresh" size={14} />{/if} 检查更新
        </button>
      {/if}
    </div>
    <div class="mt-1.5 bg-qd-surface border border-qd-border rounded-lg">
      {#each releases as r, i}
        <details class="qd-release" open={i === 0}>
          <summary><span class="font-mono text-qd-brand">v{r.version}</span><span class="text-qd-ink">{r.title}</span><span class="ml-auto text-qd-dim font-mono">{r.date}</span></summary>
          <ul>{#each r.notes as note}<li>{note}</li>{/each}</ul>
        </details>
      {/each}
    </div>
  </section>
  {#if error}<p class="qd-inline-message qd-inline-error" role="alert">{error}</p>{/if}
</div>
<style>
  .qd-release + .qd-release { border-top: 1px solid var(--color-qd-border); }
  .qd-release summary { display: flex; gap: 10px; align-items: baseline; padding: 9px 12px; font-size: 12px; cursor: pointer; list-style: none; }
  .qd-release summary::-webkit-details-marker { display: none; }
  .qd-release summary::before { content: "▸"; color: var(--color-qd-dim); transition: transform 0.15s; }
  .qd-release[open] summary::before { transform: rotate(90deg); }
  .qd-release ul { margin: 0; padding: 0 14px 10px 40px; list-style: disc; font-size: 12px; line-height: 1.75; color: var(--color-qd-body); }
  button[aria-pressed="true"] { border-color: var(--color-qd-brand); color: var(--color-qd-brand); background: var(--color-qd-brand-soft); }
</style>
