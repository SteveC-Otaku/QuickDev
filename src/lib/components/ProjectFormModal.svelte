<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { detectProject } from "../ipc";
  import type { Project } from "../types";

  interface Props {
    open: boolean;
    editing?: Project | null;
    projectCount: number;
    groups?: string[];
    onclose?: () => void;
    onsave?: (project: Project) => void | Promise<void>;
  }

  let {
    open,
    editing = null,
    projectCount,
    groups = [],
    onclose,
    onsave,
  }: Props = $props();

  let name = $state("");
  let cwd = $state("");
  let command = $state("pnpm dev");
  let portStr = $state("");
  let openUrl = $state("");
  let group = $state("");
  let error = $state("");
  let saving = $state(false);
  let detecting = $state(false);

  const isEdit = $derived(!!editing);
  const title = $derived(isEdit ? "编辑项目" : "添加项目");

  type FormDraft = {
    name: string;
    cwd: string;
    command: string;
    portStr: string;
    openUrl: string;
    group: string;
  };

  let addDraft = $state<FormDraft | null>(null);

  function hasFormContent() {
    return !!(name.trim() || cwd.trim() || portStr.trim() || openUrl.trim() || group.trim());
  }

  function saveAddDraft() {
    addDraft = { name, cwd, command, portStr, openUrl, group };
  }

  function clearAddDraft() {
    addDraft = null;
  }

  function requestClose() {
    if (!isEdit && hasFormContent()) {
      saveAddDraft();
    }
    onclose?.();
  }

  $effect(() => {
    if (!open) return;
    error = "";
    if (editing) {
      name = editing.name;
      cwd = editing.cwd;
      command = editing.command;
      portStr = editing.port?.toString() ?? "";
      openUrl = editing.openUrl ?? "";
      group = editing.group ?? "";
    } else if (addDraft) {
      name = addDraft.name;
      cwd = addDraft.cwd;
      command = addDraft.command;
      portStr = addDraft.portStr;
      openUrl = addDraft.openUrl;
      group = addDraft.group;
    } else {
      name = "";
      cwd = "";
      command = "pnpm dev";
      portStr = "";
      openUrl = "";
      group = "";
    }
  });

  function slugify(input: string) {
    const base = input
      .toLowerCase()
      .trim()
      .replace(/[^\w\u4e00-\u9fff]+/g, "-")
      .replace(/^-+|-+$/g, "");
    return `${base || "project"}-${Date.now().toString(36)}`;
  }

  function onPortInput() {
    if (/tauri/i.test(command)) return;
    const port = Number(portStr);
    if (port > 0 && port < 65536 && !openUrl.trim()) {
      openUrl = `http://localhost:${port}`;
    }
  }

  async function browseCwd() {
    try {
      const selected = await openDialog({ directory: true, multiple: false, title: "选择项目目录" });
      if (typeof selected === "string") cwd = selected;
    } catch (err) {
      error = String(err);
    }
  }

  async function runDetect() {
    const trimmedCwd = cwd.trim();
    if (!trimmedCwd) {
      error = "请先填写项目路径";
      return;
    }
    error = "";
    detecting = true;
    try {
      const result = await detectProject(trimmedCwd);
      if (result.name && !name.trim()) name = result.name;
      command = result.command;
      if (result.port) {
        portStr = String(result.port);
        openUrl = `http://localhost:${result.port}`;
      }
    } catch (err) {
      error = String(err);
    } finally {
      detecting = false;
    }
  }

  async function submit(e: Event) {
    e.preventDefault();
    error = "";

    const trimmedName = name.trim();
    const trimmedCwd = cwd.trim();
    const trimmedCommand = command.trim();

    if (!trimmedName) {
      error = "请填写项目名称";
      return;
    }
    if (!trimmedCwd) {
      error = "请填写项目路径";
      return;
    }
    if (!trimmedCommand) {
      error = "请填写启动命令";
      return;
    }

    const port = portStr.trim() ? Number(portStr) : undefined;
    if (portStr.trim() && (!port || port < 1 || port > 65535)) {
      error = "端口必须是 1–65535 之间的数字";
      return;
    }

    const project: Project = {
      id: editing?.id ?? slugify(trimmedName),
      name: trimmedName,
      cwd: trimmedCwd,
      command: trimmedCommand,
      port,
      openUrl: openUrl.trim() || (port ? `http://localhost:${port}` : undefined),
      group: group.trim() || undefined,
    };

    saving = true;
    try {
      await onsave?.(project);
      clearAddDraft();
      onclose?.();
    } catch (err) {
      error = String(err);
    } finally {
      saving = false;
    }
  }
</script>

{#if open}
  <div class="overlay" role="presentation">
    <div class="modal" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="project-form-title">
      <div class="modal-header">
        <h2 id="project-form-title" class="modal-title">{title}</h2>
        <button type="button" class="qd-btn-ghost text-base" onclick={requestClose}>✕</button>
      </div>

      <form class="modal-body" onsubmit={submit}>
        <label class="field">
          <span class="label">分组</span>
          <input
            class="input"
            bind:value={group}
            placeholder="工作 / 个人（可选）"
            list="project-group-options"
          />
          <datalist id="project-group-options">
            {#each groups as g}
              <option value={g}></option>
            {/each}
          </datalist>
        </label>

        <label class="field">
          <span class="label">项目名称</span>
          <input class="input" bind:value={name} placeholder="例如：官网前端" required />
        </label>

        <label class="field">
          <span class="label">项目路径 (cwd)</span>
          <div class="cwd-row">
            <input class="input font-mono text-xs flex-1" bind:value={cwd} placeholder="D:\Projects\my-app" required />
            <button type="button" class="qd-btn qd-btn-secondary qd-btn-sm shrink-0" onclick={browseCwd}>浏览…</button>
            <button type="button" class="qd-btn qd-btn-secondary qd-btn-sm shrink-0" disabled={detecting} onclick={runDetect}>
              {detecting ? "识别中…" : "自动识别"}
            </button>
          </div>
        </label>

        <label class="field">
          <span class="label">启动命令</span>
          <input class="input font-mono text-xs" bind:value={command} placeholder="pnpm dev" required />
        </label>

        <div class="field-row">
          <label class="field flex-1">
            <span class="label">预期端口</span>
            <input
              class="input font-mono"
              bind:value={portStr}
              oninput={onPortInput}
              placeholder="3000"
              inputmode="numeric"
            />
          </label>
          <label class="field flex-[2]">
            <span class="label">打开地址</span>
            <input class="input font-mono text-xs" bind:value={openUrl} placeholder="http://localhost:3000" />
          </label>
        </div>

        {#if !isEdit}
          <p class="hint">已有 {projectCount} 个项目 · 取消关闭后会保留未提交内容</p>
        {/if}

        {#if error}
          <p class="error">{error}</p>
        {/if}

        <div class="modal-actions">
          <button type="button" class="qd-btn qd-btn-secondary" onclick={requestClose}>取消</button>
          <button type="submit" class="qd-btn qd-btn-primary" disabled={saving}>
            {saving ? "保存中…" : isEdit ? "保存" : "添加"}
          </button>
        </div>
      </form>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 16px;
  }
  .modal {
    width: 100%;
    max-width: 440px;
    background: var(--color-qd-surface);
    border: 1px solid var(--color-qd-border);
    border-radius: 12px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.5);
  }
  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 10px;
    border-bottom: 1px solid var(--color-qd-border);
  }
  .modal-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-qd-ink);
  }
  .modal-body {
    padding: 14px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field-row {
    display: flex;
    gap: 10px;
  }
  .cwd-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .label {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-qd-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .input {
    background: var(--color-qd-elevated);
    border: 1px solid var(--color-qd-border);
    border-radius: 8px;
    padding: 8px 10px;
    font-size: 13px;
    color: var(--color-qd-ink);
    outline: none;
  }
  .input:focus {
    border-color: var(--color-qd-brand);
  }
  .hint {
    font-size: 11px;
    color: var(--color-qd-dim);
  }
  .error {
    font-size: 12px;
    color: var(--color-qd-danger);
    background: var(--color-qd-danger-soft);
    padding: 8px 10px;
    border-radius: 6px;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
