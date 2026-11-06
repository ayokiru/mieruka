import { useEffect } from "react";

interface Props {
  message: string;
  onUndo: () => void;
  onDone: () => void;
}

export function Toast({ message, onUndo, onDone }: Props) {
  useEffect(() => {
    const t = setTimeout(onDone, 8000);
    return () => clearTimeout(t);
  }, [onDone]);

  return (
    <div className="toast" role="status">
      <span>{message}</span>
      <button className="btn" onClick={onUndo}>Undo</button>
    </div>
  );
}
