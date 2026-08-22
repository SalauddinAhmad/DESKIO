/** Updates: which installed apps have a newer version published.
 *
 *  This screen *finds* updates; it does not install them. Downloading and
 *  running an installer on someone's behalf is a much larger promise than
 *  moving files to the Trash, and not one this app makes yet — so every row
 *  ends at the developer's own download page.
 */
import { useMemo, useState } from "react";
import type { UpdateInfo } from "../types";
import { UPDATE_SOURCE_LABEL } from "../types";
import { api } from "../api";
import { ArtEmpty, IconSearch, IconWarn } from "./icons";

interface Props {
  updates: UpdateInfo[];
  banner: React.ReactNode;
  loading: boolean;
  checkedApps: number;
  onRecheck: () => void;
}

export function UpdatesView({ updates, banner, loading, checkedApps, onRecheck }: Props) {
  const [query, setQuery] = useState("");
  const [showCurrent, setShowCurrent] = useState(false);
  const [focusedId, setFocusedId] = useState<string | null>(null);

  const outdated = useMemo(() => updates.filter((u) => u.outdated), [updates]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return updates
      .filter((u) => showCurrent || u.outdated)
      .filter((u) => !q || u.name.toLowerCase().includes(q));
  }, [updates, query, showCurrent]);

  const focused = updates.find((u) => u.app_id === focusedId) ?? null;

  return (
    <>
      <div className="list-col">
        <div className="list-head" data-tauri-drag-region>
          <h1 className="list-title">Updates</h1>
        </div>
        <div className="list-tools">
          <div className="search">
            <IconSearch />
            <input
              placeholder="Search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
        </div>
        {banner}
        <div className="count-line">
          {loading
            ? "Checking for updates…"
            : `${outdated.length} of ${updates.length} checked app${
                updates.length === 1 ? "" : "s"
              } out of date`}
        </div>

        <div className="list-scroll">
          {shown.map((u) => (
            <button
              key={u.app_id}
              className="card"
              aria-selected={u.app_id === focusedId}
              onClick={() => setFocusedId(u.app_id)}
            >
              <div className="card-main">
                <div className="card-name">{u.name}</div>
                <div className="card-sub">
                  {u.current_version ?? "?"}
                  {u.outdated ? " → " : " · "}
                  {u.outdated ? (
                    <span className="version-new">{u.latest_version}</span>
                  ) : (
                    "up to date"
                  )}
                </div>
              </div>
            </button>
          ))}
          {!loading && shown.length === 0 && (
            <p style={{ color: "var(--muted)", padding: "10px 12px", fontSize: 13 }}>
              {outdated.length === 0
                ? "Everything with a known source is up to date."
                : "Nothing matches that search."}
            </p>
          )}
        </div>

        <div className="list-foot">
          <label className="toggle">
            <input
              type="checkbox"
              checked={showCurrent}
              onChange={(e) => setShowCurrent(e.target.checked)}
            />
            Show up-to-date apps
          </label>
        </div>
      </div>

      <div className="detail">
        {!focused ? (
          <div className="empty">
            <ArtEmpty />
            <h2>Updates</h2>
            <p>
              Versions are read from each app's own update feed, from Homebrew, and
              from the App Store. Nothing about you is sent — each check is a public
              lookup of a name the app publishes itself.
            </p>
          </div>
        ) : (
          <>
            <div className="detail-top" data-tauri-drag-region />
            <div className="detail-body">
              <div className="hero">
                <div className="hero-main">
                  <div className="hero-name-row">
                    <div className="hero-name">{focused.name}</div>
                    <div className="hero-size">
                      {focused.outdated ? (
                        <span className="version-new">{focused.latest_version}</span>
                      ) : (
                        "Up to date"
                      )}
                    </div>
                  </div>
                  <dl className="hero-rows">
                    <div className="hero-row">
                      <dt>Installed</dt>
                      <dd>{focused.current_version ?? "Unknown"}</dd>
                    </div>
                    <div className="hero-row">
                      <dt>Latest published</dt>
                      <dd>{focused.latest_version}</dd>
                    </div>
                    <div className="hero-row">
                      <dt>According to</dt>
                      <dd>{UPDATE_SOURCE_LABEL[focused.source]}</dd>
                    </div>
                  </dl>
                </div>
              </div>

              <div className="banner" style={{ margin: "20px 0 0" }}>
                <IconWarn />
                <div>
                  <strong>BHUninstaller does not install updates</strong>
                  It tells you what is available and sends you to the developer. Running
                  an installer on your behalf is a much bigger promise than moving files
                  to the Trash, and not one this app makes.
                </div>
              </div>
            </div>
            <div className="detail-foot">
              <div style={{ color: "var(--muted)", fontSize: 13, maxWidth: "45%" }}>
                {checkedApps > 0 && `${checkedApps} app(s) had no source to check.`}
              </div>
              <div style={{ display: "flex", gap: 10, flexShrink: 0 }}>
                <button className="btn btn-ghost btn-sm" onClick={onRecheck} disabled={loading}>
                  Check again
                </button>
                <button
                  className="btn btn-primary"
                  disabled={!focused.url}
                  onClick={() => focused.url && api.openUrl(focused.url)}
                >
                  Open download page
                </button>
              </div>
            </div>
          </>
        )}
      </div>
    </>
  );
}
