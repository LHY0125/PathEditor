export function EmptyHint({ text }: { text: string }) {
  return (
    <div className="text-center py-12 text-sm" style={{ color: 'var(--app-fg)', opacity: 0.5 }}>
      {text}
    </div>
  );
}
