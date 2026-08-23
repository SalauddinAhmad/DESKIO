/** Applications: the list of what is installed, and the detail pane. */
import { useEffect, useMemo, useState } from "react";
import type { InstalledApp } from "../types";
import { api, formatDate, formatDay, humanSize } from "../api";
import { ArtEmpty, IconSearch } from "./icons";

type Sort = "name" | "size" | "newest";

interface Props {
  apps: InstalledApp[];
  icons: Record<string, string>;
  banner: React.ReactNode;
  loading: boolean;
  /** Changes when something has been removed, so the selection can be dropped. */
  removalNonce: number;
  onUninstall: (app: InstalledApp) => void;
}

export function AppsView({
  apps, icons, banner, loading, removalNonce, onUninstall,
}: Props) {
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("name");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [details, setDetails] = useState<InstalledApp | null>(null);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = q
      ? apps.filter(
          (a) =>
            a.name.toLowerCase().includes(q) ||
            (a.bundle_id ?? "").toLowerCase().includes(q)
        )
      : apps.slice();
    list.sort((a, b) => {
      if (sort === "size") return b.size_bytes - a.size_bytes;
      if (sort === "newest") return (b.modified_at ?? 0) - (a.modified_at ?? 0);
      return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    });
    return list;
  }, [apps, query, sort]);

  const selected = apps.find((a) => a.id === selectedId) ?? null;

  // A removal has happened: whatever was selected is very likely gone, so the
  // detail pane is cleared rather than left showing an app that no longer
  // exists — which previously kept offering to uninstall it again.
  useEffect(() => {
    setSelectedId(null);
    setDetails(null);
  }, [removalNonce]);

  // The detail pane's fields each cost a subprocess, so they are fetched only
  // for the app actually being looked at.
  useEffect(() => {
    let live = true;
    setDetails(null);
    if (!selectedId) return;
    api.appDetails(selectedId).then((d) => {
      if (live) setDetails(d);
    });
    return () => { live = false; };
  }, [selectedId]);

  const view = details ?? selected;

  return (
    <>
      <div className="list-col">
        <div className="list-head" data-tauri-drag-region>
          <h1 className="list-title">Applications</h1>
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
          <select
            className="sort"
            value={sort}
            onChange={(e) => setSort(e.target.value as Sort)}
          >
            <option value="name">Name</option>
            <option value="size">Size</option>
            <option value="newest">Newest</option>
          </select>
        </div>
        {banner}
        <div className="count-line">
          {loading
            ? "Scanning…"
            : `${shown.length} application${shown.length === 1 ? "" : "s"}`}
        </div>
        <div className="list-scroll">
          {shown.map((app) => (
            <button
              key={app.id}
              className="card"
              aria-selected={app.id === selectedId}
              onClick={() => setSelectedId(app.id)}
            >
              <AppIcon app={app} icons={icons} size={38} />
              <div className="card-main">
                <div className="card-name">{app.name}</div>
                <div className="card-sub">{formatDay(app.modified_at)}</div>
              </div>
              <div className="card-size">{humanSize(app.size_bytes)}</div>
            </button>
          ))}
        </div>
      </div>

      <div className="detail">
        {!view ? (
          <div className="empty">
            <ArtEmpty />
            <h2>Applications</h2>
            <p>Select an application to see what it installed and what it would leave behind.</p>
          </div>
        ) : (
          <>
            <div className="detail-top" data-tauri-drag-region />
            <div className="detail-body">
              <div className="hero">
                <AppIcon app={view} icons={icons} size={96} hero />
                <div className="hero-main">
                  <div className="hero-name-row">
                    <div className="hero-name">{view.name}</div>
                    <div className="hero-size">{humanSize(view.size_bytes)}</div>
                  </div>
                  <dl className="hero-rows">
                    <div className="hero-row">
                      <dt>Developer</dt>
                      <dd>{details ? view.publisher ?? "Unknown" : "…"}</dd>
                    </div>
                    <div className="hero-row">
                      <dt>App verification</dt>
                      <dd>
                        {!details ? "…" : view.notarized ? "Notarized" : "Not notarized"}
                      </dd>
                    </div>
                    <div className="hero-row">
                      <dt>Status</dt>
                      <dd className={view.is_running ? "link" : ""}>
                        {!details ? "…" : view.is_running ? "Running" : "Not running"}
                      </dd>
                    </div>
                  </dl>
                </div>
              </div>

              <h2 className="section-title">General</h2>
              <dl className="table">
                <Row label="Version" value={view.version ?? "—"} />
                <Row label="Identifier" value={view.bundle_id ?? "—"} />
                <Row label="Location" value={view.path ?? "—"} />
                <Row label="Modified" value={formatDate(view.modified_at)} />
                <Row label="Last opened" value={details ? formatDate(view.last_opened_at) : "…"} />
                <Row label="Created" value={formatDate(view.created_at)} />
              </dl>
            </div>
            <div className="detail-foot">
              <div style={{ color: "var(--muted)", fontSize: 13 }}>
                {view.is_running
                  ? "Quit this app before uninstalling it."
                  : "You will see everything that would be removed before anything moves."}
              </div>
              <button
                className="btn btn-primary"
                disabled={view.is_running}
                onClick={() => onUninstall(view)}
              >
                Uninstall
              </button>
            </div>
          </>
        )}
      </div>
    </>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="table-row">
      <dt>{label}</dt>
      <dd title={value}>{value}</dd>
    </div>
  );
}

function AppIcon({
  app, icons, size, hero,
}: { app: InstalledApp; icons: Record<string, string>; size: number; hero?: boolean }) {
  const data = app.icon_png_base64 ?? icons[app.id];
  const style = { width: size, height: size, flex: `0 0 ${size}px` };
  if (!data) {
    return (
      <div className={hero ? "card-icon hero-icon" : "card-icon"} style={style}>
        {app.name.slice(0, 1).toUpperCase()}
      </div>
    );
  }
  return (
    <div className={hero ? "hero-icon" : "card-icon"} style={{ ...style, background: "none" }}>
      <img src={`data:image/png;base64,${data}`} alt="" style={style} />
    </div>
  );
}
