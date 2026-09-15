import { go } from "@codemirror/lang-go";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { StreamLanguage } from "@codemirror/language";
import { c, cpp, csharp, dart, java, kotlin } from "@codemirror/legacy-modes/mode/clike";
import { swift } from "@codemirror/legacy-modes/mode/swift";
import type { Extension } from "@uiw/react-codemirror";

// CM6 has no dedicated Dart/Kotlin language package, so those (plus the other
// C-family languages) go through @codemirror/legacy-modes' clike config -
// the same underlying grammar CodeMirror 5's own dart/csharp/kotlin support
// was built on, not a downgrade from what the vanilla-JS admin.html used.
const LANG_EXTENSIONS: Record<string, Extension> = {
  c: StreamLanguage.define(c),
  cpp: StreamLanguage.define(cpp),
  csharp: StreamLanguage.define(csharp),
  java: StreamLanguage.define(java),
  kotlin: StreamLanguage.define(kotlin),
  dart: StreamLanguage.define(dart),
  swift: StreamLanguage.define(swift),
  python3: python(),
  javascript: javascript(),
  typescript: javascript({ typescript: true }),
  go: go(),
  rust: rust(),
};

export function extensionForLanguage(slug: string): Extension[] {
  const ext = LANG_EXTENSIONS[slug];
  return ext ? [ext] : [];
}
