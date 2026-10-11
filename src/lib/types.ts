export type DevStatus = "stopped" | "stopping" | "starting" | "running" | "conflict" | "crashed" | "failed" | "partial" | "rollingBack";

export interface ProjectService {
  id: string; name: string; cwd: string; command: string;
  port?: number; openUrl?: string;
  readiness?: "auto" | "process" | "port" | "http" | "window";
  healthUrl?: string; startupTimeoutSecs?: number;
  kind?: "frontend" | "backend" | "desktop" | "unknown";
}
export interface LaunchPlan {
  id: string; name: string; serviceIds: string[];
  startMode: "parallel" | "sequential"; openServiceId?: string;
}
export interface ServiceStatus {
  id: string; name: string; status: DevStatus; pid?: number; sessionId?: number;
  message?: string; openUrl?: string; readiness?: string;
}
export interface DevOperation { id: number; kind: string; startedAt: number; message: string; }
export interface ProjectStatus {
  id: string; status: DevStatus; pid?: number; services?: ServiceStatus[];
  operation?: DevOperation; revision?: number;
  activePlanIds?: string[];
}

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
  services?: ProjectService[];
  startMode?: "parallel" | "sequential";
  serviceStatuses?: ServiceStatus[];
  operation?: DevOperation;
  launchPlans?: LaunchPlan[];
  defaultPlanId?: string;
  activePlanIds?: string[];
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
  serviceId?: string;
  executable?: string;
  cwd?: string;
  sourceApp?: string;
  sourceEvidence?: string;
  parentChain?: string[];
}

export interface Settings {
  refreshIntervalSecs?: number;
  startPage?: 'ports' | 'projects' | 'settings';
  highlightPorts: number[];
  autoRefresh: boolean;
  minimizeToTray: boolean;
  autoStart: boolean;
  hotkey?: string;
  writeLogs?: boolean;
  groups?: string[];
  shell?: string;
  trayAnimation?: boolean;
}

export interface WorkspacePackage {
  name: string;
  path: string;
}

export interface CommandCandidate {
  label: string;
  cwd: string;
  command: string;
  port?: number;
  openUrl?: string;
  warning?: string;
  reason: string;
  kind?: "frontend" | "backend" | "desktop" | "unknown";
}

export interface DetectResult {
  warnings?: string[];
  cwd?: string;
  candidates?: CommandCandidate[];
  name?: string;
  command: string;
  packageManager: string;
  port?: number;
  devScript?: string;
  workspacePackages?: WorkspacePackage[];
}

export interface PortSuggest {
  cwd: string;
  name?: string;
  command: string;
  port: number;
  pid: number;
  process: string;
}

export type TabId = "ports" | "projects" | "settings";

export interface LogLine {
  text: string;
  level?: "info" | "success" | "warn" | "error";
}

export interface ConflictInfo {
  port: number;
  pid: number;
  command: string;
  projectId: string;
  planId?: string;
}
