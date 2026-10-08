/** Settings (design §07). Placeholder — implemented by the settings feature. */
export function SettingsWindow({ onClose }: { onClose: () => void }) {
  return (
    <div role="dialog" style={{ position: "fixed", inset: 80, background: "var(--win)" }} onClick={onClose}>
      Settings
    </div>
  );
}
