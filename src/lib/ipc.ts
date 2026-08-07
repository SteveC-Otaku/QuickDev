import { invoke } from "@tauri-apps/api/core";
import type { DetectResult, PortEntry, Project, Settings } from "./types";

export interface DevStatusEntry {
  id: string;
  status: string;
  pid?: number;
}

export interface StartDevResult {
  ok: boolean;
  message?: string;
  conflict?: PortEntry;
}

export const listProjects = () => invoke<Project[]>("list_projects");
export const upsertProject = (project: Project) => invoke<void>("upsert_project", { project });
export const removeProject = (id: string) => invoke<void>("remove_project", { id });
export const loadSettings = () => invoke<Settings>("load_settings");
export const updateSettings = (settings: Settings) => invoke<void>("update_settings", { settings });
export const listPorts = () => invoke<PortEntry[]>("list_ports");
export const killProcess = (pid: number) => invoke<void>("kill_process", { pid });
export const startDev = (id: string) => invoke<StartDevResult>("start_dev", { id });
export const stopDev = (id: string) => invoke<void>("stop_dev", { id });
export const stopAllDevs = () => invoke<void>("stop_all_devs");
export const listDevStatus = () => invoke<DevStatusEntry[]>("list_dev_status");
export const getDevLogs = (id: string) => invoke<string[]>("get_dev_logs", { id });
export const hideToTray = () => invoke<void>("hide_to_tray");
export const detectProject = (cwd: string) =>
  invoke<DetectResult>("detect_project", { cwd });
export const exportConfig = () => invoke<string>("export_config");
export const importConfig = (json: string) => invoke<void>("import_config", { json });
