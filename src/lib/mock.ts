import type { LogLine, PortEntry, Project } from "./types";

export const MOCK_PORTS: PortEntry[] = [
  { port: 5173, process: "个人工具", pid: 12345, command: "node .../vite/bin/vite.js", highlight: true, owned: true, projectId: "tool", projectName: "个人工具" },
  { port: 3000, process: "StarX 官网 Dev", pid: 67890, command: "node .../starx-landing/.next/server.js", highlight: true, owned: true, projectId: "starx", projectName: "StarX 官网 Dev" },
  { port: 3000, process: "node.exe", pid: 11223, command: "node .../other-app/node_modules/.bin/next dev", highlight: true, owned: false },
  { port: 8080, process: "java.exe", pid: 44556, command: "java -jar api-server.jar --port=8080" },
  { port: 2283, process: "immich.exe", pid: 33441, command: "immich-server --port 2283", highlight: true },
];

export const MOCK_PROJECTS: Project[] = [
  { id: "starx", name: "StarX 官网 Dev", cwd: "D:\\Projects\\starx-landing", command: "pnpm dev", port: 3000, status: "running", icon: "🌐" },
  { id: "tool", name: "个人工具", cwd: "D:\\Projects\\my-tool", command: "npm run dev", port: 5173, status: "running", icon: "⚡" },
  { id: "api", name: "API 测试服务", cwd: "D:\\Projects\\api-test", command: "pnpm dev --port 3000", port: 3000, status: "conflict", icon: "🔧" },
  { id: "admin", name: "后台管理", cwd: "D:\\Projects\\admin", command: "yarn dev", port: 8080, status: "stopped", icon: "📦" },
];

export const MOCK_LOGS: LogLine[] = [
  { time: "09:41:02", text: "ready in 823ms" },
  { time: "09:41:02", text: "➜ Local: http://localhost:3000", level: "success" },
  { time: "09:41:03", text: "➜ Network: http://192.168.1.8:3000", level: "info" },
  { time: "09:41:05", text: "○ Compiling /page ..." },
  { time: "09:41:06", text: "✓ Compiled /page in 1.2s", level: "success" },
  { time: "09:42:11", text: "warn - Tailwind class not found: bg-primary-50", level: "warn" },
];
