export function LoadingState({ label = "Loading…" }: { label?: string }) {
  return <div className="loading-state">{label}</div>;
}

export function ErrorState({ message }: { message: string }) {
  return <div className="error-state">Couldn't load data: {message}</div>;
}

export function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="empty-state">
      <div className="empty-state-title">{title}</div>
      <div>{body}</div>
    </div>
  );
}
