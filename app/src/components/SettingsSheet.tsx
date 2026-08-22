/** Settings.
 *
 *  Deliberately small. Everything that affects what gets removed is decided in
 *  the review sheet, where the user can see it — a preference that quietly
 *  changes removal behaviour would undermine that.
 */
import { useState } from "react";
import type { AppUpdateCheck, Settings } from "../types";
import { api } from "../api";
import { IconWarn } from "./icons";

interface Props {
  settings: Settings;
  accessGranted: boolean;
  accessApplicable: boolean;
  version: string;
  onChange: (next: Settings) => void;
  onOpenAccess: () => void;
  onUpdateFound: (check: AppUpdateCheck) => void;
  onClose: () => void;
}

export function SettingsSheet({
  settings, accessGranted, accessApplicable, version,
  onChange, onOpenAccess, onUpdateFound, onClose,
}: Props) {
  const [checking, setChecking] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const checkNow = async () => {
    setChecking(true);
    setStatus(null);
    try {
      const result = await api.checkAppUpdate(true);
      if (result.error) {
        // A failed check is never reported as "up to date" — that would be a
        // different and far more misleading claim.
        setStatus(`Could not check: ${result.error}`);
      } else if (result.update) {
        onUpdateFound(result);
      } else {
        setStatus(`You are on the latest version (${result.current}).`);
      }
    } catch (e) {
      setStatus(`Could not check: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setChecking(false);
    }
  };
  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div
        className="sheet"
        style={{ maxWidth: 620 }}
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="sheet-title">Settings</div>

        <div className="sheet-body" style={{ padding: 20 }}>
          <div className="setting-row">
            <div>
              <div className="setting-name">Play a sound while removing</div>
              <div className="setting-help">
                macOS plays its trash sound once per item, so a seven-item uninstall
                fires it seven times in a row.
              </div>
            </div>
            <label className="toggle">
              <input
                type="checkbox"
                checked={settings.removal_sound}
                onChange={(e) =>
                  onChange({ ...settings, removal_sound: e.target.checked })
                }
              />
            </label>
          </div>

          <div className="setting-row" style={{ marginTop: 12 }}>
            <div>
              <div className="setting-name">Check for updates automatically</div>
              <div className="setting-help">
                Asks GitHub once a day whether a newer BHUninstaller has been published.
                Nothing about you is sent, and nothing installs itself.
              </div>
            </div>
            <label className="toggle">
              <input
                type="checkbox"
                checked={settings.auto_check_updates}
                onChange={(e) =>
                  onChange({ ...settings, auto_check_updates: e.target.checked })
                }
              />
            </label>
          </div>

          <div className="setting-row" style={{ marginTop: 12 }}>
            <div>
              <div className="setting-name">This version</div>
              <div className="setting-help">
                {status ?? `BHUninstaller ${version}`}
              </div>
            </div>
            <button className="btn btn-ghost btn-sm" onClick={checkNow} disabled={checking}>
              {checking ? "Checking…" : "Check now"}
            </button>
          </div>

          {accessApplicable && (
            <div className="setting-row" style={{ marginTop: 12 }}>
              <div>
                <div className="setting-name">Full Disk Access</div>
                <div className="setting-help">
                  {accessGranted
                    ? "Granted. BHUninstaller can see everything it needs to."
                    : "Not granted, so some leftovers are invisible and some sizes read as unknown."}
                </div>
              </div>
              <button className="btn btn-ghost btn-sm" onClick={onOpenAccess}>
                {accessGranted ? "Review" : "Set up"}
              </button>
            </div>
          )}

          <div className="banner" style={{ margin: "16px 0 0" }}>
            <IconWarn />
            <div>
              <strong>This setting is not only a sound</strong>
              With it on, items are trashed through Finder, which is what records the
              information behind Finder's "Put Back". With it off they are trashed
              silently and "Put Back" is unavailable — nothing is lost either way,
              because BHUninstaller records where every item came from and can put a
              removal back itself from its own history.
            </div>
          </div>
        </div>

        <div className="sheet-foot" style={{ justifyContent: "flex-end" }}>
          <button className="btn btn-primary" onClick={onClose}>Done</button>
        </div>
      </div>
    </div>
  );
}
