/** Remaining Files: leftovers of apps that are no longer installed. */
import { useMemo, useState } from "react";
import type { OrphanGroup } from "../types";
import { humanSize } from "../api";
import { ArtEmpty, IconFolder, IconSearch } from "./icons";

interface Props {
  groups: OrphanGroup[];
  banner: React.ReactNode;
  loading: boolean;
  onRemove: (names: string[]) => void;
}

export function RemainingView({ groups, banner, loading, onRemove }: Props) {
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [focused, setFocused] = useState<string | null>(null);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? groups.filter((g) => g.name.toLowerCase().includes(q)) : groups;
  }, [groups, query]);

  const toggle = (name: string) => {
    const next = new Set(picked);
    next.has(name) ? next.delete(name) : next.add(name);
    setPicked(next);
  };

  const allPicked = shown.length > 0 && shown.every((g) => picked.has(g.name));
  const total = groups
    .filter((g) => picked.has(g.name))
    .reduce((n, g) => n + g.size_bytes, 0);

  const detail = groups.find((g) => g.name === focused) ?? null;

  return (
    <>
      <div className="list-col">
        <div className="list-head" data-tauri-drag-region>
          <h1 className="list-title">Remaining Files</h1>
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
            ? "Scanning for leftovers…"
            : `${groups.length} leftover group${groups.length === 1 ? "" : "s"} · ${humanSize(
                groups.reduce((n, g) => n + g.size_bytes, 0)
              )}`}
        </div>
        <div className="list-scroll">
          {shown.map((g) => (
            <div
              key={g.name}
              className="card"
              aria-selected={g.name === focused}
              onClick={() => setFocused(g.name)}
            >
              <input
                type="checkbox"
                checked={picked.has(g.name)}
                onClick={(e) => e.stopPropagation()}
                onChange={() => toggle(g.name)}
                style={{ accentColor: "var(--bh-500)", width: 15, height: 15 }}
              />
              <div
                className="card-icon"
                style={{ background: "none", color: "var(--bh-500)", width: 26, flex: "0 0 26px" }}
              >
                <IconFolder />
              </div>
              <div className="card-main">
                <div className="card-name">{g.name}</div>
                <div className="card-sub">
                  {g.items.length} item{g.items.length === 1 ? "" : "s"}
                </div>
              </div>
              <div className="card-size">{humanSize(g.size_bytes)}</div>
            </div>
          ))}
        </div>
      </div>

      <div className="detail">
        {!detail ? (
          <div className="empty">
            <ArtEmpty />
            <h2>Remaining files</h2>
            <p>
              Files left behind by applications that are no longer installed. Tick the
              ones you want to clear, then review them before anything moves.
            </p>
          </div>
        ) : (
          <>
            <div className="detail-top" data-tauri-drag-region />
            <div className="detail-body">
              <div className="hero">
                <div className="hero-main">
                  <div className="hero-name-row">
                    <div className="hero-name">{detail.name}</div>
                    <div className="hero-size">{humanSize(detail.size_bytes)}</div>
                  </div>
                  <dl className="hero-rows">
                    <div className="hero-row">
                      <dt>Items left behind</dt>
                      <dd>{detail.items.length}</dd>
                    </div>
                    <div className="hero-row">
                      <dt>Owner</dt>
                      <dd>No longer installed</dd>
                    </div>
                  </dl>
                </div>
              </div>
              <h2 className="section-title">Files</h2>
              <div className="table">
                {detail.items.map((i) => (
                  <div className="table-row" key={i.path}>
                    <dt style={{ maxWidth: "70%", overflow: "hidden", textOverflow: "ellipsis" }}>
                      {i.path}
                    </dt>
                    <dd>{humanSize(i.size_bytes, i.size_unknown)}</dd>
                  </div>
                ))}
              </div>
            </div>
          </>
        )}
        <div className="detail-foot">
          <label className="toggle">
            <input
              type="checkbox"
              checked={allPicked}
              onChange={() =>
                setPicked(allPicked ? new Set() : new Set(shown.map((g) => g.name)))
              }
            />
            Select all remaining files
          </label>
          <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
            {picked.size > 0 && (
              <span style={{ color: "var(--muted)", fontSize: 13 }}>
                {picked.size} selected · {humanSize(total)}
              </span>
            )}
            <button
              className="btn btn-primary"
              disabled={picked.size === 0}
              onClick={() => onRemove([...picked])}
            >
              Remove
            </button>
          </div>
        </div>
      </div>
    </>
  );
}
