/** A newer BHUninstaller is available.
 *
 *  The app downloads it and hands it to the file manager. It does not install
 *  anything by itself: an updater that replaces the running binary without
 *  being watched is a bigger promise than this app makes anywhere else.
 */
import { useState } from "react";
import type { AppUpdateCheck } from "../types";
import { api, humanSize } from "../api";
import { IconCheck, IconWarn } from "./icons";

interface Props {
  check: AppUpdateCheck;
  onClose: () => void;
}

export function AppUpdateSheet({ check, onClose }: Props) {
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const release = check.update;
  if (!release) return null;

  const download = async () => {
    setBusy(true);
    setError(null);
    try {
      const path = await api.downloadAppUpdate(release);
      setSaved(path);
      await api.openPath(path);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="sheet-backdrop">
      <div className="sheet" style={{ maxWidth: 620 }} role="dialog" aria-modal="true">
        <div className="sheet-title">BHUninstaller {release.version} is available</div>

        <div className="sheet-body" style={{ padding: 22 }}>
          <div className="fda-hero fda-hero-ok">
            <IconCheck />
            <div>
              <strong>
                You have {check.current} — {release.version} has been published.
              </strong>
              {release.asset_name
                ? `${release.asset_name}${
                    release.asset_size ? ` · ${humanSize(release.asset_size)}` : ""
                  }`
                : "No download for this platform in that release."}
            </div>
          </div>

          {release.notes && (
            <>
              <h2 className="section-title" style={{ fontSize: 15 }}>
                What's new
              </h2>
              <div className="release-notes">{release.notes}</div>
            </>
          )}

          {saved && (
            <div className="banner" style={{ margin: "16px 0 0", color: "var(--ok)" }}>
              <IconCheck />
              <div>
                <strong>Downloaded</strong>
                {saved}
                <br />
                Opening it now — install it as you would any other download, then
                relaunch.
              </div>
            </div>
          )}

          {error && (
            <div className="banner" style={{ margin: "16px 0 0", color: "var(--danger)" }}>
              <IconWarn />
              <div>{error}</div>
            </div>
          )}
        </div>

        <div className="sheet-foot">
          <button className="btn btn-ghost" onClick={onClose}>
            {saved ? "Done" : "Not now"}
          </button>
          <div style={{ display: "flex", gap: 10 }}>
            <button className="btn btn-ghost" onClick={() => api.openUrl(release.page_url)}>
              View release
            </button>
            <button
              className="btn btn-primary"
              onClick={download}
              disabled={busy || !release.asset_url || !!saved}
            >
              {busy ? "Downloading…" : "Download"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
