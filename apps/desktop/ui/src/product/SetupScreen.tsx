import { useEffect, useState } from "react";
import { Loader2, ShieldCheck } from "lucide-react";

import { githubSignIn, githubStatus } from "../backend";
import type { GithubStatus } from "../types";

type Props = {
  onDone: () => void;
  onSkip: () => void;
};

/**
 * First run: the one integration the product needs.
 *
 * Shows what was detected, then the device code. Signing in is the only thing
 * this screen does; there is no second step to configure.
 */
export default function SetupScreen({ onDone, onSkip }: Props) {
  const [status, setStatus] = useState<GithubStatus | null>(null);
  const [code, setCode] = useState<string | null>(null);
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

  async function signIn() {
    setBusy(true);
    setError(null);
    try {
      const next = await githubSignIn();
      setCode(next);
      const after = await githubStatus();
      setStatus(after);
      if (after.authenticated) {
        onDone();
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
          <div className="text-[10px] uppercase tracking-wide text-obd-muted">Code</div>
          <div className="mt-1 font-mono text-2xl font-semibold text-obd-accent">
            {code ?? "———————"}
          </div>
          <ol className="mt-4 space-y-1 text-[12px] text-obd-body">
            <li>1 · Copy the code</li>
            <li>2 · Approve at github.com/login/device</li>
          </ol>
          <div className="mt-3 font-mono text-[10px] text-obd-subtle">
            {status === null
              ? "checking git and gh…"
              : `git ${status.installed ? "found" : "missing"} · gh ${
                  status.authenticated ? (status.account ?? "signed in") : "not signed in"
                }`}
          </div>
        </div>

        <div className="mt-5 flex items-center gap-3">
          <button
            type="button"
            onClick={() => void signIn()}
            disabled={busy}
            className="flex items-center gap-2 rounded-lg bg-obd-accent px-4 py-2.5 text-[13px] font-semibold text-obd-on-accent disabled:opacity-60"
          >
            {busy ? <Loader2 className="animate-spin" size={14} /> : <ShieldCheck size={14} />}
            Sign in with GitHub
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
          </div>
          <div>
            <div className="text-[10px] uppercase tracking-wide text-obd-muted">Permissions</div>
            <div className="mt-1 text-obd-body">read and write repositories</div>
            <div className="text-obd-muted">nothing else</div>
          </div>
        </div>
      </div>
    </div>
  );
}
