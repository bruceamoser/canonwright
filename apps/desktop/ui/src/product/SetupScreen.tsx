import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";

import { githubSetToken, githubStatus } from "../backend";
import type { GithubStatus } from "../types";

type Props = {
  onDone: () => void;
  onSkip: () => void;
};

/**
 * First run: the one integration the product needs.
 *
 * A token rather than a CLI login — it is checked with one request, kept in the
 * OS keychain, and cannot change which account anything else on the machine uses.
 */
export default function SetupScreen({ onDone, onSkip }: Props) {
  const [status, setStatus] = useState<GithubStatus | null>(null);
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void githubStatus()
      .then((next) => {
        if (!live) {
          return;
        }
        setStatus(next);
        if (next.authenticated) {
          onDone();
        }
      })
      .catch((cause: unknown) => {
        if (live) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      });
    return () => {
      live = false;
    };
  }, [onDone]);

  async function connect() {
    setBusy(true);
    setError(null);
    try {
      const after = await githubSetToken(token);
      setStatus(after);
      if (after.authenticated) {
        onDone();
      } else {
        setError(after.detail || "GitHub did not accept that token");
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex h-full items-center justify-center bg-obd-bg px-6 text-obd-text">
      <div className="w-full max-w-2xl">
        <h1 className="text-2xl font-semibold tracking-tight">Sign in to GitHub</h1>

        <div className="mt-6 rounded-xl border border-obd-line bg-obd-surface p-5">
          <label htmlFor="gh-token" className="text-[10px] uppercase tracking-wide text-obd-muted">
            Token
          </label>
          <input
            id="gh-token"
            type="password"
            value={token}
            onChange={(event) => setToken(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && token.length > 0) {
                void connect();
              }
            }}
            placeholder="github_pat_…"
            className="mt-1.5 w-full rounded-lg border border-obd-line bg-obd-bg px-3 py-2 font-mono text-[12px] text-obd-text outline-none focus:border-obd-accent"
          />
          <div className="mt-2 font-mono text-[10px] text-obd-subtle">
            github.com/settings/personal-access-tokens
          </div>
        </div>

        <div className="mt-5 flex items-center gap-3">
          <button
            type="button"
            onClick={() => void connect()}
            disabled={busy || token.trim().length === 0}
            className="flex items-center gap-2 rounded-lg bg-obd-accent px-4 py-2.5 text-[13px] font-semibold text-obd-on-accent disabled:opacity-60"
          >
            {busy && <Loader2 className="animate-spin" size={14} />}
            Connect
          </button>
          <button
            type="button"
            onClick={onSkip}
            className="rounded-lg border border-obd-line px-4 py-2.5 text-[12px] text-obd-muted hover:bg-obd-raised"
          >
            Continue without
          </button>
        </div>

        {error !== null && (
          <p className="mt-4 rounded-lg border border-obd-danger/40 bg-obd-raised px-3 py-2 text-[11px] text-obd-danger">
            {error}
          </p>
        )}

        <div className="mt-8 grid grid-cols-2 gap-4 border-t border-obd-line pt-6 text-[11px]">
          <div>
            <div className="text-[10px] uppercase tracking-wide text-obd-muted">Signed in as</div>
            <div className="mt-1 text-obd-body">{status?.account ?? "Not signed in"}</div>
            <div className="mt-1 font-mono text-[10px] text-obd-subtle">
              git {status?.installed === true ? "found" : "missing"}
            </div>
          </div>
          <div>
            <div className="text-[10px] uppercase tracking-wide text-obd-muted">Permissions</div>
            <div className="mt-1 text-obd-body">contents: read and write</div>
            <div className="text-obd-muted">issues: read and write</div>
          </div>
        </div>
      </div>
    </div>
  );
}
