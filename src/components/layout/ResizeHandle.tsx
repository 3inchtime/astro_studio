import { useState } from "react";

interface ResizeHandleProps {
  onMouseDown: (e: React.MouseEvent) => void;
}

export function ResizeHandle({ onMouseDown }: ResizeHandleProps) {
  const [isDragging, setIsDragging] = useState(false);

  return (
    <div
      onMouseDown={(e) => {
        setIsDragging(true);
        onMouseDown(e);
        const onUp = () => {
          setIsDragging(false);
          document.removeEventListener("mouseup", onUp);
        };
        document.addEventListener("mouseup", onUp);
      }}
      className={`
        relative z-10 w-0.5 shrink-0 cursor-col-resize transition-colors duration-150
        hover:bg-primary/20
        ${isDragging ? "bg-primary/40" : "bg-transparent"}
      `}
      style={{ margin: "0 -1px" }}
    />
  );
}
