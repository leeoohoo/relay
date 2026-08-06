import type { ReactNode } from "react";

export function FlowStep(props: { index: string; title: string; detail: string }) {
  return <div><span>{props.index}</span><p><strong>{props.title}</strong><small>{props.detail}</small></p></div>;
}

export function Field(props: { label: string; children: ReactNode }) {
  return <label className="field"><span>{props.label}</span>{props.children}</label>;
}

export function NavItem(props: {
  icon: string;
  label: string;
  badge?: number;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button className={props.active ? "active" : ""} onClick={props.onClick}>
      <Icon name={props.icon} />
      {props.label}
      {props.badge ? <span className="nav-badge">{props.badge > 99 ? "99+" : props.badge}</span> : null}
    </button>
  );
}

export function Icon({ name }: { name: string }) {
  const paths: Record<string, ReactNode> = {
    network: <><circle cx="6" cy="6" r="2"/><circle cx="18" cy="6" r="2"/><circle cx="12" cy="18" r="2"/><path d="m7.7 7 3.2 8M16.3 7l-3.2 8M8 6h8"/></>,
    plus: <path d="M12 5v14M5 12h14"/>, key: <><circle cx="8" cy="12" r="3"/><path d="M11 12h9M17 12v3M20 12v2"/></>,
    git: <><circle cx="6" cy="4" r="2"/><circle cx="18" cy="7" r="2"/><circle cx="6" cy="20" r="2"/><path d="M6 6v12M8 7c5 0 3 0 8 0M13 7v4c0 4-2 5-5 5"/></>,
    tasks: <><rect x="4" y="3" width="16" height="18" rx="2"/><path d="m8 8 1.5 1.5L12 7M14 9h3M8 14l1.5 1.5L12 13M14 15h3"/></>,
    memory: <><path d="M9 4a3 3 0 0 0-5 2.2A3.5 3.5 0 0 0 4.5 13 3.5 3.5 0 0 0 9 18.4V4zM15 4a3 3 0 0 1 5 2.2 3.5 3.5 0 0 1-.5 6.8 3.5 3.5 0 0 1-4.5 5.4V4z"/><path d="M9 8H7M15 8h2M9 13H6.5M15 13h2.5M12 3v18"/></>,
    search: <><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></>,
    terminal: <><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M13 15h4"/></>,
    plugin: <><path d="M8 3v4a2 2 0 0 1-2 2H3v6h3a2 2 0 0 1 2 2v4h6v-3a2 2 0 0 1 2-2h5v-6h-4a2 2 0 0 1-2-2V3z"/><path d="M11 3v3M21 13h-3M11 21v-3M3 12h3"/></>,
    book: <><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/><path d="M8 7h8M8 11h6"/></>,
    folder: <><path d="M3 6a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3z"/><path d="M3 9h18"/></>,
    paperclip: <path d="m21.4 11.6-8.9 8.9a6 6 0 0 1-8.5-8.5l9.6-9.6a4 4 0 0 1 5.7 5.7l-9.6 9.6a2 2 0 0 1-2.8-2.8l8.9-8.9"/>,
    image: <><rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="8.5" cy="9" r="1.5"/><path d="m21 15-5-5L5 20"/></>,
    download: <><path d="M12 3v12M7 10l5 5 5-5"/><path d="M5 21h14"/></>,
    org: <><rect x="9" y="3" width="6" height="5" rx="1"/><rect x="3" y="16" width="6" height="5" rx="1"/><rect x="15" y="16" width="6" height="5" rx="1"/><path d="M12 8v4M6 16v-4h12v4"/></>,
    message: <path d="M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4z"/>, group: <><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"/></>,
    logout: <><path d="M10 17l5-5-5-5M15 12H3"/><path d="M14 3h5a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-5"/></>, refresh: <><path d="M20 11a8.1 8.1 0 0 0-15.5-2M4 4v5h5"/><path d="M4 13a8.1 8.1 0 0 0 15.5 2M20 20v-5h-5"/></>,
    settings: <><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.6v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1z"/></>,
    pause: <><path d="M9 5v14M15 5v14"/></>, play: <path d="m8 5 11 7-11 7z"/>, trash: <><path d="M3 6h18M8 6V4h8v2M19 6l-1 15H6L5 6M10 11v6M14 11v6"/></>,
    "chevron-down": <path d="m6 9 6 6 6-6"/>, "chevron-up": <path d="m18 15-6-6-6 6"/>, "chevron-left": <path d="m15 18-6-6 6-6"/>, "chevron-right": <path d="m9 18 6-6-6-6"/>, "arrow-left": <path d="m19 12H5m6 6-6-6 6-6"/>, check: <path d="m5 12 4 4L19 6"/>, shield: <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>,
    close: <path d="M18 6 6 18M6 6l12 12"/>, alert: <><path d="M10.3 2.9 1.8 17a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 2.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01"/></>,
    eye: <><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></>, copy: <><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></>,
  };
  return <svg viewBox="0 0 24 24" aria-hidden="true">{paths[name] ?? paths.network}</svg>;
}
