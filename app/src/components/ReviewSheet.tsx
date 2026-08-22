/** The review sheet — the most important screen in the product.
 *
 *  Nothing is ever removed without this being shown first, and it is not
 *  skippable. Every row carries its full path and the reason the engine matched
 *  it, so the decision is the user's and not ours.
 */
import { useMemo, useState } from "react";
import type { RemovalItem, RemovalPlan, RemovalReport } from "../types";
import { humanSize } from "../api";
import { IconTrash, IconWarn } from "./icons";

interface Props {
  plan: RemovalPlan;
  busy: boolean;
  error: string | null;
  report: RemovalReport | null;
  expert: boolean;
  onToggle: (path: string) => void;
  onCancel: () => void;
  onConfirm: () => void;
  onForce: () => void;
}

export function ReviewSheet(props: Props) {
  const { plan, busy, error, report, expert, onToggle, onCancel, onConfirm, onForce } = props;
  const [showWeak, setShowWeak] = useState(false);

  // Low-confidence rows are hidden by default so the list reads as the things
  // we are actually confident about — but never silently: the count is always
  // shown, and one click reveals them.
  const strong = useMemo(() => plan.items.filter((i) => i.confidence !== "low"), [plan.items]);
  const weak = useMemo(() => plan.items.filter((i) => i.confidence === "low"), [plan.items]);
  const visible = expert || showWeak ? plan.items : strong;

  const selected = plan.items.filter((i) => i.selected);
  const bytes = selected.reduce((n, i) => n + i.size_bytes, 0);
  const anyUnknown = selected.some((i) => i.size_unknown);
  const needsAdmin = selected.some((i) => i.requires_admin);

  if (report) {
    return <ResultView report={report} busy={busy} onForce={onForce} onClose={onCancel} />;
  }

  return (
    <div className="sheet-backdrop">
      <div className="sheet" role="dialog" aria-modal="true">
        <div className="sheet-title">Review and confirm removal of selected items</div>

        <div className="sheet-strip">
          <div>
            <b>{plan.app ? plan.app.name : "Leftover files"}</b>{" "}
            <span className="muted">
              {plan.app ? "uninstalling" : `from ${plan.items.length} location(s)`}
            </span>
          </div>
          <b>{humanSize(bytes, anyUnknown)}</b>
        </div>

        <div className="sheet-body">
          {visible.map((item) => (
            <Row key={item.path} item={item} onToggle={onToggle} disabled={busy} />
          ))}

          {!expert && !showWeak && weak.length > 0 && (
            <button className="disclosure" onClick={() => setShowWeak(true)}>
              Show {weak.length} more item{weak.length === 1 ? "" : "s"} that only loosely
              match this app
            </button>
          )}

          {plan.delegated_command && (
            <div className="banner" style={{ margin: "10px 4px 0" }}>
              <IconWarn />
              <div>
                <strong>This app's own uninstaller runs first</strong>
                <code style={{ display: "block", margin: "4px 0", userSelect: "text" }}>
                  {plan.delegated_command}
                </code>
                It knows how to undo what its installer did. Nothing below is touched
                unless it finishes successfully.
              </div>
            </div>
          )}

          {needsAdmin && (
            <div className="banner" style={{ margin: "10px 4px 0" }}>
              <IconWarn />
              <div>
                <strong>Some items need an administrator password</strong>
                They sit outside your home folder. They will be moved into a dated
                BHUninstaller folder in your Trash, which you can open and inspect.
                Finder's "Put Back" will not work for those — the removal history keeps
                a record of where each one came from.
              </div>
            </div>
          )}

          {error && (
            <div className="banner" style={{ margin: "10px 4px 0", color: "var(--danger)" }}>
              <IconWarn />
              <div>{error}</div>
            </div>
          )}
        </div>

        <div className="sheet-foot">
          <button className="btn btn-ghost" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <div className="sheet-tally">
            <div>
              <span className="n">{humanSize(bytes, anyUnknown).split(" ")[0]}</span>
              <span className="u">{humanSize(bytes, anyUnknown).split(" ")[1] ?? ""} selected</span>
            </div>
            <div>
              <span className="n">{selected.length}</span>
              <span className="u">items selected</span>
            </div>
          </div>
          <button
            className="btn btn-primary"
            onClick={onConfirm}
            disabled={busy || selected.length === 0}
          >
            <IconTrash />
            {busy ? "Removing…" : "Remove"}
          </button>
        </div>
      </div>
    </div>
  );
}

function Row({
  item, onToggle, disabled,
}: { item: RemovalItem; onToggle: (p: string) => void; disabled: boolean }) {
  return (
    <label className="item">
      <input
        type="checkbox"
        checked={item.selected}
        disabled={disabled}
        onChange={() => onToggle(item.path)}
      />
      <div className="item-main">
        <div className="item-name">{item.name}</div>
        <div className="item-path">{item.path}</div>
        <div className="item-why">
          {item.confidence === "medium" && <span className="pill pill-medium">likely</span>}
          {item.confidence === "low" && <span className="pill pill-low">uncertain</span>}
          {item.requires_admin && <span className="pill pill-admin">admin</span>}
          {item.reason}
        </div>
      </div>
      <div className="item-size">{humanSize(item.size_bytes, item.size_unknown)}</div>
    </label>
  );
}

function ResultView({
  report, busy, onForce, onClose,
}: {
  report: RemovalReport;
  busy: boolean;
  onForce: () => void;
  onClose: () => void;
}) {
  const removed = report.outcomes.filter((o) => o.removed);
  const failed = report.outcomes.filter((o) => !o.removed);
  return (
    <div className="sheet-backdrop">
      <div className="sheet" role="dialog" aria-modal="true">
        <div className="sheet-title">
          {report.delegated_failed ? "Nothing was removed" : "Removal complete"}
        </div>
        <div className="sheet-strip">
          <div>
            <b>{removed.length}</b> <span className="muted">item(s) moved to the Trash</span>
          </div>
          <b>{humanSize(report.bytes_freed)}</b>
        </div>
        <div className="sheet-body">
          {report.delegated_failed && (
            <div className="banner" style={{ margin: "4px 4px 12px" }}>
              <IconWarn />
              <div>
                <strong>{report.delegated_failed}</strong>
                Nothing was touched, because removing an app's files underneath its
                own half-finished uninstaller usually leaves a worse mess than
                stopping. If that uninstaller is simply broken, you can clear the
                files anyway.
              </div>
            </div>
          )}
          {failed.length > 0 && !report.delegated_failed && (
            <>
              <div className="section-title" style={{ marginTop: 4, fontSize: 15 }}>
                Not removed
              </div>
              {failed.map((o) => (
                <div className="result-line bad" key={o.path}>
                  <IconWarn />
                  <div>
                    <div className="item-path" style={{ color: "inherit" }}>{o.path}</div>
                    <div>{o.error}</div>
                  </div>
                </div>
              ))}
            </>
          )}
          {failed.length === 0 && (
            <p style={{ color: "var(--muted)", padding: "8px 12px" }}>
              Everything selected was moved to the Trash. Nothing was deleted — you can
              put it back from the History screen, or from the Trash itself.
              <br />
              <br />
              Your disk gets the space back when you empty the Trash.
            </p>
          )}
        </div>
        <div className="sheet-foot" style={{ justifyContent: "flex-end", gap: 10 }}>
          {report.delegated_failed && (
            <button className="btn btn-ghost" onClick={onForce} disabled={busy}>
              {busy ? "Removing…" : "Remove the files anyway"}
            </button>
          )}
          <button className="btn btn-primary" onClick={onClose}>Done</button>
        </div>
      </div>
    </div>
  );
}
