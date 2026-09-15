import type { ReactNode } from "react";

interface ModalProps {
  title: string;
  narrow?: boolean;
  onClose: () => void;
  footer: ReactNode;
  children: ReactNode;
}

/** Ports admin.html's .modal-backdrop/.modal pattern: click the backdrop or the × to close. */
export default function Modal({ title, narrow, onClose, footer, children }: ModalProps) {
  return (
    <div
      className="modal-backdrop"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className={"modal" + (narrow ? " narrow" : "")}>
        <div className="modal-head">
          <h3>{title}</h3>
          <button className="modal-close" onClick={onClose} type="button">
            &times;
          </button>
        </div>
        <div className="modal-body">{children}</div>
        <div className="modal-foot">{footer}</div>
      </div>
    </div>
  );
}
