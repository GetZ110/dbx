export function isTauriRuntime(globalObject: Record<string, unknown> = globalThis as Record<string, unknown>): boolean {
  return Boolean(globalObject.__TAURI_INTERNALS__ || globalObject.__TAURI__);
}

/**
 * HarmonyOS desktop-like runtime marker injected by the ArkUI shell.
 * The web app should enable desktop UI features but keep using the HTTP backend.
 */
export function isHarmonyDesktopRuntime(globalObject: Record<string, unknown> = globalThis as Record<string, unknown>): boolean {
  return Boolean(globalObject.__HARMONY_DESKTOP__);
}

/**
 * True when the UI should behave as a desktop app: real Tauri or HarmonyOS
 * desktop-like runtime.
 */
export function isDesktopRuntime(globalObject: Record<string, unknown> = globalThis as Record<string, unknown>): boolean {
  return isTauriRuntime(globalObject) || isHarmonyDesktopRuntime(globalObject);
}
