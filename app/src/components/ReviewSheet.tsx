/** The review sheet — the most important screen in the product.
 *
 *  Nothing is ever removed without this being shown first, and it is not
 *  skippable. Every row carries its full path and the reason the engine matched
 *  it, so the decision is the user's and not ours.
 */
import { useMemo, useState } from "react";
import type { RemovalItem, RemovalPlan, RemovalReport } from "../types";
import { humanSize, TRASH_NAME } from "../api";
import { IconCheck, IconTrash, IconWarn } from "./icons";

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
    return (
      <ResultView
        report={report}
        appName={plan.app?.name ?? null}
        busy={busy}
        onForce={onForce}
        onClose={onCancel}
      />
    );
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

          {selected.some((i) => i.registry_key) && (
            <div className="banner" style={{ margin: "10px 4px 0" }}>
              <IconWarn />
              <div>
                <strong>Registry keys are backed up before they are removed</strong>
                A registry key cannot go to the {TRASH_NAME}, so each one is exported to a
                .reg file first — that export is what the History screen puts back. A key
                is never deleted unless its backup was written successfully.
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
      <div className="item-size">
        {item.registry_key ? "Registry key" : humanSize(item.size_bytes, item.size_unknown)}
      </div>
    </label>
  );
}

function ResultView({
  report, appName, busy, onForce, onClose,
}: {
  report: RemovalReport;
  appName: string | null;
  busy: boolean;
  onForce: () => void;
  onClose: () => void;
}) {
  const moved = report.outcomes.filter((o) => o.removed && !o.already_gone);
  const alreadyGone = report.outcomes.filter((o) => o.already_gone);
  const failed = report.outcomes.filter((o) => !o.removed);
  const name = appName;

  // What actually happened, in one sentence, without making the user work it
  // out from three numbers. The case that used to read as failure — "0 items
  // moved to the Trash, Zero KB" — is the app's own uninstaller having already
  // done the job, which is a complete success and now says so.
  let headline: string;
  let detail: string;

  if (report.delegated_failed) {
    headline = name
      ? `${name}'s own uninstaller did not finish`
      : "The application's own uninstaller did not finish";
    detail =
      "Nothing was removed. This usually means the uninstaller was cancelled, or it " +
      "needed a password it did not get. You can try again, or clear the files anyway.";
  } else if (failed.length > 0 && moved.length === 0) {
    headline = "Nothing was removed";
    detail =
      failed.length === 1
        ? "One item could not be removed. It is listed below."
        : `${failed.length} items could not be removed. They are listed below.`;
  } else if (moved.length === 0 && alreadyGone.length > 0) {
    headline = name ? `${name} was already gone` : "Everything was already gone";
    detail = report.delegated_ran
      ? "Its own uninstaller had removed everything, and there was nothing left behind to clear up."
      : "Everything selected had already been removed, so there was nothing left to do.";
  } else {
    const count = `${moved.length} item${moved.length === 1 ? "" : "s"}`;
    headline = name ? `${name} was removed` : `${count} removed`;
    // "freeing Zero KB" is the same confusion as the headline had: it reads as
    // though nothing happened. When there is no size worth reporting, the
    // clause is simply left off.
    const freed = report.bytes_freed > 0 ? `, freeing ${humanSize(report.bytes_freed)}` : "";
    if (report.delegated_ran) {
      detail = `Its own uninstaller removed the application. BHUninstaller cleared ${count} it left behind${freed}.`;
    } else {
      detail = `${count} moved to the ${TRASH_NAME}${freed}.`;
    }
    if (failed.length === 0) {
      detail += " Nothing was left behind.";
    }
  }

  return (
    <div className="sheet-backdrop">
      <div className="sheet" style={{ maxWidth: 640 }} role="dialog" aria-modal="true">
        <div className="sheet-title">{headline}</div>

        <div className="sheet-body" style={{ padding: 22 }}>
          <div className={failed.length > 0 && moved.length === 0 ? "result-hero bad" : "result-hero"}>
            {report.delegated_failed || (failed.length > 0 && moved.length === 0) ? (
              <IconWarn />
            ) : (
              <IconCheck />
            )}
            <div>
              <div className="result-headline">
                {moved.length > 0
                  ? report.bytes_freed > 0
                    ? humanSize(report.bytes_freed)
                    : "Zero KB remaining"
                  : report.delegated_failed
                    ? "Not finished"
                    : "Nothing to remove"}
              </div>
              <div className="result-detail">{detail}</div>
            </div>
          </div>

          {failed.length > 0 && !report.delegated_failed && (
            <>
              <h2 className="section-title" style={{ fontSize: 14 }}>
                {failed.length === 1 ? "One item was left alone" : `${failed.length} items were left alone`}
              </h2>
              <div className="table" style={{ padding: "2px 14px" }}>
                {failed.map((o) => (
                  <div className="result-line bad" key={o.path}>
                    <IconWarn />
                    <div style={{ minWidth: 0 }}>
                      <div className="item-path" style={{ marginTop: 0, color: "inherit" }}>
                        {o.path}
                      </div>
                      <div>{o.error}</div>
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}

          {moved.length > 0 && (
            <p className="result-note">
              Everything is in the {TRASH_NAME}, not deleted — put it back from the History
              screen if you change your mind. Your disk gets the space back when you empty
              the {TRASH_NAME}.
            </p>
          )}
        </div>

        <div className="sheet-foot" style={{ justifyContent: "flex-end", gap: 10 }}>
          {report.delegated_failed && (
            <button className="btn btn-ghost" onClick={onForce} disabled={busy}>
              {busy ? "Removing…" : "Remove the files anyway"}
            </button>
          )}
          <button className="btn btn-primary" onClick={onClose} disabled={busy}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
