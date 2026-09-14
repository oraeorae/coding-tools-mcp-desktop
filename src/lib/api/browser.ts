import { invoke } from "@tauri-apps/api/core";
export interface BrowserStatus { state: string; message: string; managed: boolean; nodeAvailable: boolean; appVersion: string; toolCount: number }
export const browserStatus = (id: string) => invoke<BrowserStatus>("browser_status", { id });
export const startBrowserBridge = (id: string) => invoke<BrowserStatus>("start_browser_bridge", { id });
export const stopBrowserBridge = (id: string) => invoke<BrowserStatus>("stop_browser_bridge", { id });
