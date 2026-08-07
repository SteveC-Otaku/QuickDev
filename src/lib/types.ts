export type DevStatus = "stopped" | "starting" | "running" | "conflict" | "crashed";

export interface Project {
  id: string;
  name: string;
  cwd: string;
  command: string;
  port?: number;
  openUrl?: string;
  tags?: string[];
  group?: string;
  status?: DevStatus;
  icon?: string;
}

export interface PortEntry {
  port: number;
  process: string;
  pid: number;
  command: string;
  highlight?: boolean;
  owned?: boolean;
  projectId?: string;
  projectName?: string;
}

export interface Settings {
  highlightPorts: number[];
  autoRefresh: boolean;
  minimizeToTray: boolean;
  autoStart: boolean;
  hotkey?: string;
  writeLogs?: boolean;
  groups?: string[];
  shell?: string;
}

export interface DetectResult {
  name?: string;
  command: string;
  packageManager: string;
  port?: number;
  devScript?: string;
}

export type TabId = "ports" | "projects" | "settings";

export interface LogLine {
  time: string;
  text: string;
  level?: "info" | "success" | "warn" | "error";
}
