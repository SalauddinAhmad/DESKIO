/** The Full Disk Access explanation.
 *
 *  macOS gives an app no way to request this permission — only to point at
 *  System Settings. So the job here is to be worth granting: say exactly what
 *  is hidden right now, exactly what changes if it is granted, and exactly what
 *  the app does with the access. Then get out of the way.
 *
 *  It appears once, on first launch, and never again on its own.
 */
import { useState } from "react";
import type { AccessReport } from "../types";
import { api } from "../api";
import { IconCheck, IconLock, IconWarn } from "./icons";

interface Props {
  report: AccessReport;
  onRecheck: () => Promise<AccessReport>;
  onClose: () => void;
}

export function FullDiskAccessSheet({ report, onRecheck, onClose }: Props) {
  const [checking, setChecking] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);
  const [latest, setLatest] = useState(report);

  // If the pane cannot be opened, say so and give the steps — a button that
  // quietly does nothing is worse than one that admits it failed.
  const openSettings = async () => {
    setOpenFailed(false);
    try {
      await api.openPrivacySettings();
    } catch {
      setOpenFailed(true);
    }
  };

  const recheck = async () => {
    setChecking(true);
    try {
      setLatest(await onRecheck());
    } finally {
      setChecking(false);
    }
  };

  // Granted while the app was already running: macOS decides this per process,
  // so the running one still cannot see anything until it is restarted.
  if (latest.granted) {
    return (
      <div className="sheet-backdrop">
        <div className="sheet" style={{ maxWidth: 620 }} role="dialog" aria-modal="true">
          <div className="sheet-title">Full Disk Access granted</div>
          <div className="sheet-body" style={{ padding: 24 }}>
            <div className="fda-hero fda-hero-ok">
              <IconCheck />
              <div>
                <strong>That's the permission sorted.</strong>
                macOS applies it to newly started apps only, so BHUninstaller needs to
                start again before it can actually see anything.
              </div>
            </div>
          </div>
          <div className="sheet-foot" style={{ justifyContent: "flex-end", gap: 10 }}>
            <button className="btn btn-ghost" onClick={onClose}>
              Later
            </button>
            <button className="btn btn-primary" onClick={() => api.relaunch()}>
              Relaunch now
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="sheet-backdrop">
      <div className="sheet" style={{ maxWidth: 720 }} role="dialog" aria-modal="true">
        <div className="sheet-title">Full Disk Access</div>

        <div className="sheet-body" style={{ padding: 24 }}>
          <div className="fda-hero">
            <IconLock />
            <div>
              <strong>macOS is hiding part of your Library from BHUninstaller.</strong>
              An app cannot ask for this permission — it has to be granted in System
              Settings. Here is exactly what it costs to leave it off.
            </div>
          </div>

          {latest.blocked.length > 0 ? (
            <>
              <h2 className="section-title" style={{ fontSize: 15 }}>
                Hidden right now — {latest.blocked.length} of {latest.checked} locations
              </h2>
              <div className="table">
                {latest.blocked.map((b) => (
                  <div key={b.path} style={{ padding: "11px 0" }} className="fda-row">
                    <div className="item-path" style={{ marginTop: 0 }}>
                      {b.path}
                    </div>
                    <div style={{ fontSize: 12.5, color: "var(--ink-2)", marginTop: 3 }}>
                      {b.consequence}
                    </div>
                  </div>
                ))}
              </div>
            </>
          ) : (
            <p style={{ color: "var(--muted)" }}>
              Nothing is being withheld at the moment, but the permission is not granted,
              so anything protected that appears later would be missed.
            </p>
          )}

          <h2 className="section-title" style={{ fontSize: 15 }}>
            How to grant it
          </h2>
          <ol className="fda-steps">
            <li>Open System Settings › Privacy &amp; Security › Full Disk Access.</li>
            <li>
              Find <b>BHUninstaller</b> in the list, or add it with the <b>+</b> button if
              it is not there.
            </li>
            <li>Turn its switch on.</li>
            <li>Come back here and press <b>Check again</b>.</li>
          </ol>

          {openFailed && (
            <div className="banner" style={{ margin: "18px 0 0", color: "var(--danger)" }}>
              <IconWarn />
              <div>
                <strong>System Settings did not open</strong>
                Open it yourself: <b>System Settings › Privacy &amp; Security › Full Disk
                Access</b>, then come back and press Check again.
              </div>
            </div>
          )}

          <div className="banner" style={{ margin: "18px 0 0" }}>
            <IconWarn />
            <div>
              <strong>What BHUninstaller does with it</strong>
              Reads those folders to work out what belongs to which app, and nothing else.
              It never sends anything anywhere — the one time it uses the network is the
              Updates screen, which looks up published version numbers and tells them
              nothing about you.
            </div>
          </div>
        </div>

        <div className="sheet-foot">
          <button className="btn btn-ghost" onClick={onClose}>
            Continue without it
          </button>
          <div style={{ display: "flex", gap: 10 }}>
            <button className="btn btn-ghost" onClick={recheck} disabled={checking}>
              {checking ? "Checking…" : "Check again"}
            </button>
            <button className="btn btn-primary" onClick={openSettings}>
              Open Privacy &amp; Security
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
