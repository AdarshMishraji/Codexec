import { useState } from "react";

interface ChipInputProps {
  values: string[];
  onChange: (values: string[]) => void;
  placeholder?: string;
}

/** Ports admin.html's chip-input: type a token, Enter/comma commits it as a chip, Backspace on an empty field pops the last one. Used for compile_cmd/run_cmd argv arrays. */
export default function ChipInput({ values, onChange, placeholder }: ChipInputProps) {
  const [draft, setDraft] = useState("");

  function commit() {
    const v = draft.trim();
    if (v) {
      onChange([...values, v]);
      setDraft("");
    }
  }

  return (
    <div className="chip-input">
      {values.map((v, i) => (
        <span className="chip" key={i}>
          {v}{" "}
          <button type="button" onClick={() => onChange(values.filter((_, idx) => idx !== i))}>
            &times;
          </button>
        </span>
      ))}
      <input
        type="text"
        value={draft}
        placeholder={placeholder ?? "type a token, press Enter"}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === ",") {
            e.preventDefault();
            commit();
          } else if (e.key === "Backspace" && !draft && values.length) {
            onChange(values.slice(0, -1));
          }
        }}
      />
    </div>
  );
}
