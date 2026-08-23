/** Mirrors `bhu-core::model`. Kept in sync by hand — the shapes are small and
 *  change rarely, and a codegen step would be more machinery than it earns. */

export type Confidence = "high" | "medium" | "low";

export type LeftoverKind =
  | "preferences" | "caches" | "application_support" | "container"
  | "group_container" | "logs" | "saved_state" | "launch_agent"
  | "launch_daemon" | "privileged_helper" | "cookies" | "web_data"
  | "receipt" | "extension" | "crash_report" | "registry_key" | "other";

export interface InstalledApp {
  id: string;
  name: string;
  path: string | null;
  bundle_id: string | null;
  executable: string | null;
  version: string | null;
  publisher: string | null;
  size_bytes: number;
  source: string;
  icon_png_base64: string | null;
  created_at: number | null;
  modified_at: number | null;
  last_opened_at: number | null;
  notarized: boolean | null;
  is_running: boolean;
  is_system: boolean;
  /** "All users" or "You" on Windows; absent elsewhere. */
  scope: string | null;
}

export interface RemovalItem {
  path: string;
  name: string;
  size_bytes: number;
  size_unknown: boolean;
  is_directory: boolean;
  kind: LeftoverKind;
  confidence: Confidence;
  /** Why the engine believes this belongs to the app. Always shown. */
  reason: string;
  requires_admin: boolean;
  selected: boolean;
  /** Set when this is a registry key rather than a file. */
  registry_key: string | null;
}

export interface RemovalPlan {
  app: InstalledApp | null;
  items: RemovalItem[];
  delegated_command: string | null;
}

export interface Leftover {
  path: string;
  name: string;
  size_bytes: number;
  size_unknown: boolean;
  is_directory: boolean;
  kind: LeftoverKind;
  confidence: Confidence;
  reason: string;
  requires_admin: boolean;
  shared_with: string[];
  registry_key: string | null;
}

export interface OrphanGroup {
  name: string;
  items: Leftover[];
  size_bytes: number;
}

export interface RemovalOutcome {
  path: string;
  removed: boolean;
  already_gone: boolean;
  error: string | null;
}

export interface RemovalReport {
  outcomes: RemovalOutcome[];
  bytes_freed: number;
  undo_id: string | null;
  /** Set when the app's own uninstaller failed and the sweep was abandoned. */
  delegated_failed: string | null;
  /** The app's own uninstaller ran and finished. */
  delegated_ran: boolean;
}

export type StartupKind =
  | "launch_agent" | "launch_daemon" | "login_item" | "registry_run" | "autostart";

export interface StartupItem {
  id: string;
  name: string;
  kind: StartupKind;
  path: string | null;
  program: string | null;
  enabled: boolean;
  can_toggle: boolean;
  locked_reason: string | null;
  requires_admin: boolean;
  app_id: string | null;
}

export const STARTUP_KIND_LABEL: Record<StartupKind, string> = {
  launch_agent: "Launch Agent",
  launch_daemon: "System Daemon",
  login_item: "User Login Item",
  registry_run: "Registry Run Entry",
  autostart: "Autostart Entry",
};

export type ExtensionCategory =
  | "installation_files" | "browser_extension" | "screen_saver"
  | "settings_pane" | "internet_plugin" | "widget";

export interface ExtensionItem {
  id: string;
  name: string;
  category: ExtensionCategory;
  path: string;
  size_bytes: number;
  size_unknown: boolean;
  is_directory: boolean;
  requires_admin: boolean;
  detail: string | null;
}

export interface ExtensionGroup {
  category: ExtensionCategory;
  label: string;
  description: string;
  items: ExtensionItem[];
  size_bytes: number;
}

export type JunkCategory =
  | "app_caches" | "logs" | "crash_reports" | "developer_junk" | "trash";

export interface JunkItem {
  id: string;
  name: string;
  category: JunkCategory;
  path: string;
  size_bytes: number;
  size_unknown: boolean;
  is_directory: boolean;
  requires_admin: boolean;
  detail: string | null;
}

export interface JunkGroup {
  category: JunkCategory;
  label: string;
  description: string;
  removable: boolean;
  items: JunkItem[];
  size_bytes: number;
}

export type UpdateSource = "sparkle" | "homebrew_cask" | "mac_app_store";

export const UPDATE_SOURCE_LABEL: Record<UpdateSource, string> = {
  sparkle: "The developer's own update feed",
  homebrew_cask: "Homebrew",
  mac_app_store: "Mac App Store",
};

export interface UpdateInfo {
  app_id: string;
  name: string;
  current_version: string | null;
  latest_version: string;
  source: UpdateSource;
  url: string | null;
  outdated: boolean;
}

export interface UndoItem {
  original: string;
  trashed: string | null;
}

export interface UndoEntry {
  id: string;
  timestamp: number;
  app_name: string | null;
  items: UndoItem[];
  paths: string[];
  bytes_freed: number;
  restorable: number;
}

export interface RestoreOutcome {
  original: string;
  restored: boolean;
  error: string | null;
}

export interface BlockedLocation {
  path: string;
  consequence: string;
}

export interface AccessReport {
  granted: boolean;
  checked: number;
  blocked: BlockedLocation[];
  applicable: boolean;
}

export interface AppRelease {
  version: string;
  notes: string;
  page_url: string;
  asset_name: string | null;
  asset_url: string | null;
  asset_size: number;
}

export interface AppUpdateCheck {
  update: AppRelease | null;
  current: string;
  error: string | null;
}

export interface Settings {
  removal_sound: boolean;
  full_disk_prompt_seen: boolean;
  auto_check_updates: boolean;
  last_update_check: number;
}

export type Section =
  | "applications" | "startup" | "extensions" | "remaining"
  | "updates" | "cleanup" | "history";
