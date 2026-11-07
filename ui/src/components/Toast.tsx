import { useEffect } from "react";

interface Props {
  message: string;
  // Errors stay until dismissed and are announced right away; other messages fade after a while.
  error?: boolean;
  onUndo?: () => void;
  onDone: () => void;
}

export function Toast({ message, error, onUndo, onDone }: Props) {
  useEffect(() => {
    if (error) return;
    const t = setTimeout(onDone, onUndo ? 8000 : 4000);
    return () => clearTimeout(t);
  }, [onDone, error, onUndo]);

  return (
    <div className={error ? "toast error" : "toast"} role={error ? "alert" : "status"}>
      <span>{message}</span>
      {onUndo && <button className="btn" onClick={onUndo}>Undo</button>}
      {error && <button className="btn" onClick={onDone}>Dismiss</button>}
    </div>
  );
}
