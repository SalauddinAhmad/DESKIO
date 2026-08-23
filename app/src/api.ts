/** The bridge to the engine.
 *
 *  Outside Tauri — i.e. `npm run dev` in a plain browser — the same calls are
 *  served from a fixture captured from a real machine, so the interface can be
 *  worked on without a native window. Anything destructive refuses to run
 *  there: a design session must never be able to move a real file.
 */
import type {
  AccessReport, AppRelease, AppUpdateCheck, ExtensionGroup, InstalledApp,
  JunkGroup, OrphanGroup, RemovalPlan, RemovalReport,
  RestoreOutcome, Settings, StartupItem, UndoEntry, UpdateInfo,
} from "./types";

export const IS_TAURI =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

let fixture: any = null;
async function fixtures() {
  if (fixture) return fixture;
  // Resolved at runtime, not build time: the fixture is captured from a real
  // machine and deliberately not committed, so it may simply not be there.
  const path = "./fixtures.json";
  try {
    fixture = (await import(/* @vite-ignore */ path)).default;
  } catch {
    fixture = { apps: [], orphans: [], plans: {} };
  }
  return fixture;
}

export const api = {
  async listApps(refresh = false): Promise<InstalledApp[]> {
    if (!IS_TAURI) return (await fixtures()).apps;
    return call("list_apps", { refresh });
  },

  async appIcons(): Promise<Record<string, string>> {
    if (!IS_TAURI) return {};
    return call("app_icons");
  },

  async appDetails(id: string): Promise<InstalledApp | null> {
    if (!IS_TAURI) {
      const f = await fixtures();
      // The captured plans hold the enriched record, which is closer to what
      // the real detail pane shows than the bare list entry.
      return f.plans[id]?.app ?? f.apps.find((a: InstalledApp) => a.id === id) ?? null;
    }
    return call("app_details", { id });
  },

  async planUninstall(id: string): Promise<RemovalPlan | null> {
    if (!IS_TAURI) return (await fixtures()).plans[id] ?? null;
    return call("plan_uninstall", { id });
  },

  async orphanGroups(refresh = false): Promise<OrphanGroup[]> {
    if (!IS_TAURI) return (await fixtures()).orphans;
    return call("orphan_groups", { refresh });
  },

  async planOrphans(names: string[]): Promise<RemovalPlan> {
    if (!IS_TAURI) {
      const groups: OrphanGroup[] = (await fixtures()).orphans;
      const items = groups
        .filter((g) => names.includes(g.name))
        .flatMap((g) => g.items)
        .map((l) => ({ ...l, selected: true }));
      return { app: null, items, delegated_command: null } as RemovalPlan;
    }
    return call("plan_orphans", { names });
  },

  async executePlan(plan: RemovalPlan, force = false): Promise<RemovalReport> {
    if (!IS_TAURI) {
      throw new Error(
        "Running in a browser without the engine — nothing can be removed here."
      );
    }
    return call("execute_plan", { plan, force });
  },

  async startupItems(refresh = false): Promise<StartupItem[]> {
    if (!IS_TAURI) return (await fixtures()).startup ?? [];
    return call("startup_items", { refresh });
  },

  async setStartupEnabled(id: string, enabled: boolean): Promise<void> {
    if (!IS_TAURI) {
      throw new Error("Running in a browser without the engine — nothing can change here.");
    }
    return call("set_startup_enabled", { id, enabled });
  },

  async extensionGroups(refresh = false): Promise<ExtensionGroup[]> {
    if (!IS_TAURI) return (await fixtures()).extensions ?? [];
    return call("extension_groups", { refresh });
  },

  async planExtensions(ids: string[]): Promise<RemovalPlan> {
    if (!IS_TAURI) {
      const groups: ExtensionGroup[] = (await fixtures()).extensions ?? [];
      const items = groups
        .flatMap((g) => g.items)
        .filter((i) => ids.includes(i.id))
        .map((i) => ({
          path: i.path, name: i.name, size_bytes: i.size_bytes,
          size_unknown: i.size_unknown, is_directory: i.is_directory,
          kind: "extension", confidence: "medium",
          reason: i.detail ?? "", requires_admin: i.requires_admin, selected: true,
        }));
      return { app: null, items, delegated_command: null } as RemovalPlan;
    }
    return call("plan_extensions", { ids });
  },

  async junkGroups(refresh = false): Promise<JunkGroup[]> {
    if (!IS_TAURI) return (await fixtures()).junk ?? [];
    return call("junk_groups", { refresh });
  },

  async planCleanup(ids: string[]): Promise<RemovalPlan> {
    if (!IS_TAURI) {
      const groups: JunkGroup[] = (await fixtures()).junk ?? [];
      const items = groups
        .filter((g) => g.removable)
        .flatMap((g) => g.items)
        .filter((i) => ids.includes(i.id))
        .map((i) => ({
          path: i.path, name: i.name, size_bytes: i.size_bytes,
          size_unknown: i.size_unknown, is_directory: i.is_directory,
          kind: "caches", confidence: "medium",
          reason: i.detail ?? "", requires_admin: i.requires_admin, selected: true,
        }));
      return { app: null, items, delegated_command: null } as RemovalPlan;
    }
    return call("plan_cleanup", { ids });
  },

  async checkUpdates(refresh = false): Promise<UpdateInfo[]> {
    if (!IS_TAURI) return (await fixtures()).updates ?? [];
    return call("check_updates", { refresh });
  },

  async removalHistory(): Promise<UndoEntry[]> {
    if (!IS_TAURI) return (await fixtures()).history ?? [];
    return call("removal_history");
  },

  async restoreRemoval(id: string): Promise<RestoreOutcome[]> {
    if (!IS_TAURI) {
      // Preview mode: answer as the engine would, but touch nothing.
      const entries: UndoEntry[] = (await fixtures()).history ?? [];
      const entry = entries.find((e) => e.id === id);
      return (entry?.items ?? []).map((i) => ({
        original: i.original,
        restored: true,
        error: null,
      }));
    }
    return call("restore_removal", { id });
  },

  async checkAppUpdate(force: boolean): Promise<AppUpdateCheck> {
    if (!IS_TAURI) return { update: null, current: "0.1.0", error: null };
    return call("check_app_update", { force });
  },

  async downloadAppUpdate(release: AppRelease): Promise<string> {
    if (!IS_TAURI) throw new Error("Not available in preview mode.");
    return call("download_app_update", { release });
  },

  /// Start the downloaded installer; the app closes itself so it can be
  /// replaced. Handled in Rust rather than through the opener plugin, which
  /// refuses open_path unless it is granted in the capability file.
  async installUpdate(path: string): Promise<void> {
    if (!IS_TAURI) return;
    return call("install_update", { path });
  },

  async getSettings(): Promise<Settings> {
    if (!IS_TAURI) {
      return {
        removal_sound: false,
        full_disk_prompt_seen: true,
        auto_check_updates: false,
        last_update_check: 0,
      };
    }
    return call("get_settings");
  },

  async setSettings(settings: Settings): Promise<void> {
    if (!IS_TAURI) return;
    return call("set_settings", { settings });
  },

  async fullDiskAccess(): Promise<boolean> {
    if (!IS_TAURI) return true;
    return call("full_disk_access");
  },

  async accessReport(): Promise<AccessReport> {
    if (!IS_TAURI) {
      return { granted: true, checked: 0, blocked: [], applicable: false };
    }
    return call("access_report");
  },

  async openPrivacySettings(): Promise<void> {
    if (!IS_TAURI) return;
    return call("open_privacy_settings");
  },

  async relaunch(): Promise<void> {
    if (!IS_TAURI) return;
    return call("relaunch");
  },

  async openUrl(url: string): Promise<void> {
    if (!IS_TAURI) return;
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  },
};

/** Byte sizes in the reference app's style: "180.5 MB", "Zero KB". */
export function humanSize(bytes: number, unknown = false): string {
  if (unknown) return "unknown";
  if (bytes === 0) return "Zero KB";
  const KB = 1000;
  if (bytes < KB) return `${bytes} bytes`;
  const units: [number, string][] = [
    [KB, "KB"],
    [KB * KB, "MB"],
    [KB * KB * KB, "GB"],
  ];
  let value = bytes / KB;
  let unit = "KB";
  for (const [div, u] of units) {
    if (bytes < div * KB) {
      value = bytes / div;
      unit = u;
      break;
    }
    value = bytes / (KB * KB * KB);
    unit = "GB";
  }
  return `${value >= 100 ? Math.round(value) : value.toFixed(1)} ${unit}`;
}

export function formatDate(unixSeconds: number | null): string {
  if (!unixSeconds) return "—";
  return new Date(unixSeconds * 1000).toLocaleString(undefined, {
    day: "numeric", month: "short", year: "numeric",
    hour: "numeric", minute: "2-digit",
  });
}

export function formatDay(unixSeconds: number | null): string {
  if (!unixSeconds) return "—";
  return new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
    day: "numeric", month: "short", year: "numeric",
  });
}

/** What this platform calls the trash, so the interface uses the user's word. */
export const TRASH_NAME =
  typeof navigator !== "undefined" && /Win/i.test(navigator.platform ?? "")
    ? "Recycle Bin"
    : "Trash";

/** A size for an installed application.
 *
 *  An app that really occupies no space does not exist, so a zero here always
 *  means the size could not be determined — on Windows, an install location the
 *  registry does not record and a size it does not estimate. Printing
 *  "Zero KB" for that is worse than admitting we do not know.
 */
export function appSize(bytes: number): string {
  return bytes > 0 ? humanSize(bytes) : "—";
}
