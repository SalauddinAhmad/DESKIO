/** History: what has been removed, and putting it back.
 *
 *  This is the counterweight to everything else in the app. Every removal is
 *  recorded here with where each item came from and where it went, so a change
 *  of mind never depends on remembering what happened.
 */
import { useState } from "react";
import type { RestoreOutcome, UndoEntry } from "../types";
import { humanSize, formatDate } from "../api";
import { ArtEmpty, IconWarn } from "./icons";

interface Props {
  entries: UndoEntry[];
  loading: boolean;
  busyId: string | null;
  /// The outcome of the last restore, carrying the id of the entry it was for.
  result: { id: string; outcomes: RestoreOutcome[] } | null;
  onRestore: (id: string) => void;
}

export function HistoryView({ entries, loading, busyId, result, onRestore }: Props) {
  const [focusedId, setFocusedId] = useState<string | null>(null);
  const focused = entries.find((e) => e.id === focusedId) ?? null;
  // Shown only against the removal it actually describes.
  const outcomes = focused && result?.id === focused.id ? result.outcomes : null;

  const itemsOf = (e: UndoEntry) =>
    e.items.length > 0
      ? e.items
      : e.paths.map((p) => ({ original: p, trashed: null }));

  // From the engine, which checks the trash rather than trusting the journal.
  const restorable = focused?.restorable ?? 0;

  return (
    <>
      <div className="list-col">
        <div className="list-head" data-tauri-drag-region>
          <h1 className="list-title">History</h1>
        </div>
        <div className="count-line">
          {loading
            ? "Reading…"
            : `${entries.length} removal${entries.length === 1 ? "" : "s"}`}
        </div>
        <div className="list-scroll">
          {entries.map((e) => {
            const items = itemsOf(e);
            return (
              <button
                key={e.id}
                className="card"
                aria-selected={e.id === focusedId}
                onClick={() => setFocusedId(e.id)}
              >
                <div className="card-main">
                  <div className="card-name">{e.app_name ?? "Leftover files"}</div>
                  <div className="card-sub">
                    {formatDate(e.timestamp)} · {items.length} item
                    {items.length === 1 ? "" : "s"}
                  </div>
                </div>
                <div className="card-size">{humanSize(e.bytes_freed)}</div>
              </button>
            );
          })}
          {!loading && entries.length === 0 && (
            <p style={{ color: "var(--muted)", padding: "10px 12px", fontSize: 13 }}>
              Nothing has been removed yet.
            </p>
          )}
        </div>
      </div>

      <div className="detail">
        {!focused ? (
          <div className="empty">
            <ArtEmpty />
            <h2>History</h2>
            <p>
              Everything BHUninstaller has removed, with where each item came from.
              Select a removal to look through it or put it back.
            </p>
          </div>
        ) : (
          <>
            <div className="detail-top" data-tauri-drag-region />
            <div className="detail-body">
              <div className="hero">
                <div className="hero-main">
                  <div className="hero-name-row">
                    <div className="hero-name">{focused.app_name ?? "Leftover files"}</div>
                    <div className="hero-size">{humanSize(focused.bytes_freed)}</div>
                  </div>
                  <dl className="hero-rows">
                    <div className="hero-row">
                      <dt>Removed</dt>
                      <dd>{formatDate(focused.timestamp)}</dd>
                    </div>
                    <div className="hero-row">
                      <dt>Items</dt>
                      <dd>{itemsOf(focused).length}</dd>
                    </div>
                    <div className="hero-row">
                      <dt>Can be put back</dt>
                      <dd>{restorable}</dd>
                    </div>
                  </dl>
                </div>
              </div>

              {outcomes && (
                <div
                  className="banner"
                  style={{
                    margin: "16px 0 0",
                    color: outcomes.every((o) => o.restored) ? "var(--ok)" : "var(--warn)",
                  }}
                >
                  <IconWarn />
                  <div>
                    <strong>
                      Put back {outcomes.filter((o) => o.restored).length} of{" "}
                      {outcomes.length} item(s)
                    </strong>
                    {outcomes
                      .filter((o) => !o.restored)
                      .slice(0, 5)
                      .map((o) => (
                        <div key={o.original}>
                          {o.original.split("/").pop()}: {o.error}
                        </div>
                      ))}
                  </div>
                </div>
              )}

              {restorable === 0 && !outcomes && (
                <div className="banner" style={{ margin: "16px 0 0" }}>
                  <IconWarn />
                  <div>
                    {itemsOf(focused).every((i) => i.trashed === null)
                      ? "This removal predates BHUninstaller recording where items went, so it cannot put them back for you. Anything still in the Trash can be moved out of it by hand — the original paths are listed below."
                      : "Nothing from this removal is in the Trash any more — it was either already put back or the Trash has been emptied."}
                  </div>
                </div>
              )}

              <h2 className="section-title">Items</h2>
              <div style={{ display: "grid", gap: 2 }}>
                {itemsOf(focused).map((i) => (
                  <div className="item" key={i.original}>
                    <div className="item-main">
                      <div className="item-path" style={{ marginTop: 0 }}>
                        {i.original}
                      </div>
                      {!i.trashed && (
                        <div className="item-why">
                          <span className="pill pill-low">no record</span>
                          where this went was not recorded
                        </div>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
            <div className="detail-foot">
              <div style={{ color: "var(--muted)", fontSize: 13 }}>
                Items already back in place are skipped rather than overwritten.
              </div>
              <button
                className="btn btn-primary"
                disabled={restorable === 0 || busyId === focused.id}
                onClick={() => onRestore(focused.id)}
              >
                {busyId === focused.id ? "Putting back…" : "Put back"}
              </button>
            </div>
          </>
        )}
      </div>
    </>
  );
}
