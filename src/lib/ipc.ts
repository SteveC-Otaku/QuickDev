import { invoke } from "@tauri-apps/api/core";
import type { DetectResult, PortEntry, PortSuggest, Project, Settings, ProjectStatus } from "./types";

import { normalizeProject } from "./services";

export type DevStatusEntry = ProjectStatus;

export interface StartDevResult {
  ok: boolean;
  message?: string;
  conflict?: PortEntry;
}

export interface BootstrapPayload {
  settings: Settings;
  projects: Project[];
  statuses: DevStatusEntry[];
}

export const bootstrap = async () => {
  const payload = await invoke<BootstrapPayload>("bootstrap");
  return {...payload, projects:payload.projects.map(normalizeProject)};
};
export const listProjects = async () => (await invoke<Project[]>("list_projects")).map(normalizeProject);
export const upsertProject = (project: Project) => invoke<void>("upsert_project", { project });
export const removeProject = (id: string) => invoke<void>("remove_project", { id });
export const loadSettings = () => invoke<Settings>("load_settings");
export const updateSettings = (settings: Settings) => invoke<void>("update_settings", { settings });
export const listPorts = () => invoke<PortEntry[]>("list_ports");
export const killProcess = (pid: number) => invoke<void>("kill_process", { pid });
export const killAndRestart = (pid: number, projectId: string, planId?:string) =>
  invoke<StartDevResult>("kill_and_restart", { pid, projectId, planId });
export const startDev = (id: string, planId?: string) => invoke<StartDevResult>("start_dev", { id, planId });
export const startService = (id: string, serviceId: string) => invoke<StartDevResult>("start_service", { id, serviceId });
export const stopService = (id: string, serviceId: string) => invoke<void>("stop_service", { id, serviceId });
export const focusService = (id: string, serviceId: string) => invoke<void>("focus_service", { id, serviceId });
export const openServiceEntry = (id: string, serviceId: string) => invoke<void>("open_service", { id, serviceId });
export const stopDev = (id: string, planId?: string) => invoke<void>("stop_dev", { id, planId });
export const stopAllDevs = () => invoke<void>("stop_all_devs");
export const listDevStatus = () => invoke<DevStatusEntry[]>("list_dev_status");
export const openDataDirectory = (kind: 'config' | 'logs') => invoke<void>('open_data_directory', { kind });
export const getDevLogs = (id: string, serviceId?: string) => invoke<string[]>("get_dev_logs", { id, serviceId });
export const detectProject = (cwd: string) => invoke<DetectResult>("detect_project", { cwd });
export const suggestProjects = () => invoke<PortSuggest[]>("suggest_projects");
export const exportConfig = () => invoke<string>("export_config");
export const importConfig = (json: string) => invoke<void>("import_config", { json });
