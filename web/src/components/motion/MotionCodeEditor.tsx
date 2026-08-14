import { useEffect, useRef } from "react";
import { Compartment } from "@codemirror/state";
import { css } from "@codemirror/lang-css";
import { html } from "@codemirror/lang-html";
import { oneDark } from "@codemirror/theme-one-dark";
import { basicSetup, EditorView } from "codemirror";
import type { MotionDocumentFile } from "../../lib/types";

interface MotionCodeEditorProps {
  file: MotionDocumentFile;
  value: string;
  label: string;
  onChange: (value: string) => void;
}

const fillEditor = EditorView.theme({
  "&": { height: "100%", backgroundColor: "#0d0f13" },
  ".cm-scroller": { overflow: "auto", fontFamily: "var(--font-mono)" },
  ".cm-content": { padding: "12px 0", caretColor: "var(--accent-primary)" },
  ".cm-gutters": {
    backgroundColor: "#0d0f13",
    color: "var(--text-muted)",
    borderRight: "var(--bw-thin) solid var(--border-subtle)",
  },
  ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "rgba(255,255,255,.035)" },
  ".cm-selectionBackground, ::selection": { backgroundColor: "rgba(142,139,255,.32) !important" },
});

function languageFor(file: MotionDocumentFile) {
  return file === "index.html" ? html({ matchClosingTags: true, autoCloseTags: true }) : css();
}

/** One controlled CodeMirror instance whose language compartment follows the selected source. */
export function MotionCodeEditor({ file, value, label, onChange }: MotionCodeEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const languageRef = useRef(new Compartment());
  const onChangeRef = useRef(onChange);
  const externalUpdateRef = useRef(false);
  const currentFileRef = useRef(file);
  const selectionByFileRef = useRef(new Map<MotionDocumentFile, { anchor: number; head: number }>());

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const view = new EditorView({
      parent: host,
      doc: value,
      extensions: [
        basicSetup,
        oneDark,
        fillEditor,
        languageRef.current.of(languageFor(file)),
        EditorView.lineWrapping,
        EditorView.updateListener.of((update) => {
          if (update.docChanged && !externalUpdateRef.current) {
            onChangeRef.current(update.state.doc.toString());
          }
        }),
      ],
    });
    view.contentDOM.setAttribute("aria-label", label);
    view.contentDOM.setAttribute("aria-multiline", "true");
    viewRef.current = view;
    return () => {
      viewRef.current = null;
      view.destroy();
    };
    // The controlled effects below update file, value, label, and callback.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const previousFile = currentFileRef.current;
    const currentSelection = view.state.selection.main;
    if (previousFile !== file) {
      selectionByFileRef.current.set(previousFile, {
        anchor: currentSelection.anchor,
        head: currentSelection.head,
      });
    }
    const savedSelection = previousFile === file
      ? { anchor: currentSelection.anchor, head: currentSelection.head }
      : selectionByFileRef.current.get(file) ?? { anchor: 0, head: 0 };
    const selection = {
      anchor: Math.min(savedSelection.anchor, value.length),
      head: Math.min(savedSelection.head, value.length),
    };
    const changes = view.state.doc.toString() === value
      ? undefined
      : { from: 0, to: view.state.doc.length, insert: value };
    if (!changes && previousFile === file) return;
    externalUpdateRef.current = true;
    try {
      view.dispatch({
        changes,
        effects: previousFile === file
          ? undefined
          : languageRef.current.reconfigure(languageFor(file)),
        selection,
      });
      currentFileRef.current = file;
    } finally {
      externalUpdateRef.current = false;
    }
  }, [file, value]);

  useEffect(() => {
    viewRef.current?.contentDOM.setAttribute("aria-label", label);
  }, [label]);

  return <div ref={hostRef} data-motion-editor data-file={file} className="motion-code-editor" />;
}
