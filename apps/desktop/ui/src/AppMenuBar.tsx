import { useEffect, useRef, useState } from "react";
import { Check, Monitor, Moon, Sun } from "lucide-react";

import { useTheme } from "./theme";

export type AppMenuItem = {
  label: string;
  shortcut?: string;
  onSelect?: () => void;
  disabled?: boolean;
  checked?: boolean;
  separator?: boolean;
};

export type AppMenu = {
  label: string;
  items: AppMenuItem[];
};

export default function AppMenuBar({ menus }: { menus: AppMenu[] }) {
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  const theme = useTheme();
  const container = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const close = (event: PointerEvent) => {
      if (
        container.current !== null &&
        !container.current.contains(event.target as Node)
      ) {
        setOpenMenu(null);
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpenMenu(null);
      }
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", escape);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", escape);
    };
  }, []);

  return (
    <div
      ref={container}
      role="menubar"
      aria-label="Application menu"
      className="relative z-40 flex h-8 shrink-0 items-center gap-0.5 border-b border-obd-line bg-obd-bg px-2"
    >
      <div className="mr-2 flex items-center gap-1.5 px-1">
        <img
          src="/canonwright-icon.svg"
          alt=""
          aria-hidden="true"
          className="h-4 w-4 rounded-[4px]"
        />
        <span className="text-[10px] font-medium tracking-tight text-obd-muted">
          Canonwright
        </span>
      </div>

      {menus.map((menu) => {
        const open = openMenu === menu.label;
        return (
          <div key={menu.label} className="relative">
            <button
              type="button"
              role="menuitem"
              aria-haspopup="menu"
              aria-expanded={open}
              onClick={() => setOpenMenu(open ? null : menu.label)}
              onMouseEnter={() => {
                if (openMenu !== null) {
                  setOpenMenu(menu.label);
                }
              }}
              className={`rounded px-2 py-1 text-[10px] ${
                open
                  ? "bg-obd-raised-strong text-obd-strong"
                  : "text-obd-muted hover:bg-obd-raised-strong hover:text-obd-text"
              }`}
            >
              {menu.label}
            </button>
            {open && (
              <div
                role="menu"
                aria-label={`${menu.label} menu`}
                className="absolute left-0 top-full mt-1 w-64 overflow-hidden rounded-lg border border-obd-line bg-obd-raised p-1 shadow-2xl shadow-black/50"
              >
                {menu.items.map((item, index) =>
                  item.separator === true ? (
                    <div
                      key={`separator-${index}`}
                      role="separator"
                      className="my-1 border-t border-obd-line"
                    />
                  ) : (
                    <button
                      key={item.label}
                      type="button"
                      role="menuitem"
                      disabled={item.disabled === true}
                      onClick={() => {
                        setOpenMenu(null);
                        item.onSelect?.();
                      }}
                      className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[11px] text-obd-body hover:bg-obd-raised-strong hover:text-obd-strong disabled:cursor-not-allowed disabled:opacity-35"
                    >
                      <span className="flex w-3 shrink-0 justify-center text-obd-accent">
                        {item.checked === true ? <Check size={11} /> : null}
                      </span>
                      <span className="min-w-0 flex-1 truncate">{item.label}</span>
                      {item.shortcut !== undefined && (
                        <span className="shrink-0 font-mono text-[9px] text-obd-subtle">
                          {item.shortcut}
                        </span>
                      )}
                    </button>
                  ),
                )}
              </div>
            )}
          </div>
        );
      })}
      <button
        type="button"
        onClick={theme.cycle}
        title={`Theme: ${theme.choice} — click for the next`}
        aria-label={`Theme: ${theme.choice}`}
        className="ml-auto mr-1 flex items-center gap-1 rounded px-1.5 py-1 text-[10px] text-obd-subtle hover:bg-obd-raised hover:text-obd-body"
      >
        {theme.choice === "system" ? (
          <Monitor size={12} />
        ) : theme.choice === "light" ? (
          <Sun size={12} />
        ) : (
          <Moon size={12} />
        )}
        <span className="capitalize">{theme.choice}</span>
      </button>
    </div>
  );
}
