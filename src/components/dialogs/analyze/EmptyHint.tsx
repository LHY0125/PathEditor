export function EmptyHint({ text }: { text: string }) {
  return (
    <div className="text-center py-12 text-xs" style={{ color: 'var(--app-fg-subtle)' }}>
      {text}
    </div>
  );
}
