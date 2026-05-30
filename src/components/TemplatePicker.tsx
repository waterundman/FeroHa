import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import FeroHaIcon from "./FeroHaIcon";
import { renderTemplate } from "../lib/promptTemplate";

interface TemplateMeta {
  path: string;
  title: string;
  preview: string;
}

interface TemplatePickerProps {
  isOpen: boolean;
  onClose: () => void;
  onSelectTemplate: (content: string, fileName: string) => void;
  isTauri: boolean;
}

const BLANK_TEMPLATE = { path: "", title: "Blank Note", preview: "# {{title}}" };

function TemplatePickerInner({ onClose, onSelectTemplate, isTauri }: Omit<TemplatePickerProps, 'isOpen'>) {
  const [templates, setTemplates] = useState<TemplateMeta[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [userTitle, setUserTitle] = useState("");
  const [userTags, setUserTags] = useState("");
  const [fileName, setFileName] = useState("");
  const [loadedContent, setLoadedContent] = useState("# {{title}}\n\n");
  const listRef = useRef<HTMLDivElement>(null);
  const titleRef = useRef<HTMLInputElement>(null);
  const tagsRef = useRef<HTMLInputElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const allTemplates = useMemo(() => [BLANK_TEMPLATE, ...templates], [templates]);

  useEffect(() => {
    if (isTauri) {
      import("@tauri-apps/api/core")
        .then(({ invoke }) => invoke<TemplateMeta[]>("list_templates"))
        .then((list) => setTemplates(list))
        .catch(() => setTemplates([]));
    }
  }, [isTauri]);

  const loadTemplateContent = useCallback((idx: number, currentAllTemplates: TemplateMeta[]) => {
    if (idx === 0) {
      setLoadedContent("# {{title}}\n\n");
      return;
    }
    if (!isTauri) return;
    const tmpl = currentAllTemplates[idx];
    if (!tmpl || !tmpl.path) return;
    import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke<string>("read_note", { path: tmpl.path }))
      .then((content) => setLoadedContent(content))
      .catch(() => setLoadedContent(""));
  }, [isTauri]);

  const handleSelect = useCallback((idx: number) => {
    setSelectedIndex(idx);
    loadTemplateContent(idx, allTemplates);
  }, [loadTemplateContent, allTemplates]);

  const hasTitle = loadedContent.includes("{{title}}");
  const hasTags = loadedContent.includes("{{tags}}");
  const needsTitleInput = hasTitle;
  const needsTagsInput = hasTags;
  const needsFileInput = !hasTitle && !hasTags;

  useEffect(() => {
    if (hasTitle) {
      titleRef.current?.focus();
    } else if (needsFileInput) {
      fileRef.current?.focus();
    } else if (hasTags) {
      tagsRef.current?.focus();
    }
  }, [hasTitle, needsFileInput, hasTags, loadedContent]);

  const handleConfirm = useCallback(() => {
    const title = userTitle.trim();
    const tags = userTags.trim();
    const file = fileName.trim();

    let finalFileName: string;
    if (hasTitle) {
      if (!title) return;
      finalFileName = title.replace(/[<>:"/\\|?*\n\r]/g, "-") + ".md";
    } else if (needsFileInput) {
      if (!file) return;
      finalFileName = file.endsWith(".md") ? file : file + ".md";
    } else {
      if (!file) return;
      finalFileName = file.endsWith(".md") ? file : file + ".md";
    }

    const variables: Record<string, unknown> = {
      title: title || file.replace(/\.md$/, ""),
      date: new Date().toISOString().slice(0, 10),
      time: new Date().toTimeString().slice(0, 5),
      tags: tags
        ? tags.split(",").map((t) => t.trim()).filter(Boolean)
        : [],
    };

    const rendered = renderTemplate(loadedContent, variables);
    onSelectTemplate(rendered, finalFileName);
  }, [userTitle, userTags, fileName, hasTitle, needsFileInput, loadedContent, onSelectTemplate]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }
      if (e.key === "Enter" && e.target === e.currentTarget) {
        e.preventDefault();
        handleConfirm();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => {
          const next = Math.min(prev + 1, allTemplates.length - 1);
          loadTemplateContent(next, allTemplates);
          return next;
        });
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => {
          const next = Math.max(prev - 1, 0);
          loadTemplateContent(next, allTemplates);
          return next;
        });
      }
    },
    [onClose, handleConfirm, allTemplates, loadTemplateContent]
  );

  return (
    <div style={styles.overlay} onClick={onClose}>
      <div style={styles.dialog} onClick={(e) => e.stopPropagation()} onKeyDown={handleKeyDown}>
        <div style={styles.header}>
          <span style={styles.title}>New Note — Choose Template</span>
          <button style={styles.closeBtn} onClick={onClose} title="Close (Esc)">
            <FeroHaIcon name="X" size={16} />
          </button>
        </div>

        <div style={styles.body}>
          <div style={styles.list} ref={listRef}>
            {allTemplates.map((tmpl, idx) => (
              <div
                key={tmpl.path || "__blank__"}
                style={{
                  ...styles.item,
                  ...(idx === selectedIndex ? styles.itemActive : {}),
                }}
                onClick={() => handleSelect(idx)}
                onMouseEnter={() => handleSelect(idx)}
              >
                <span style={styles.itemTitle}>
                  <FeroHaIcon name={idx === 0 ? "FileText" : "FilePen"} size={14} /> {tmpl.title}
                </span>
                <span style={styles.itemPreview}>
                  {idx === 0 ? "Default blank note with title" : tmpl.preview.slice(0, 200)}
                </span>
              </div>
            ))}
          </div>

          <div style={styles.inputs}>
            {needsTitleInput && (
              <div style={styles.field}>
                <label style={styles.label}>Title</label>
                <input
                  ref={titleRef}
                  style={styles.input}
                  value={userTitle}
                  onChange={(e) => setUserTitle(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      handleConfirm();
                    }
                  }}
                  placeholder="Enter note title"
                />
              </div>
            )}
            {needsTagsInput && (
              <div style={styles.field}>
                <label style={styles.label}>Tags (comma-separated)</label>
                <input
                  ref={tagsRef}
                  style={styles.input}
                  value={userTags}
                  onChange={(e) => setUserTags(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      handleConfirm();
                    }
                  }}
                  placeholder="tag1, tag2, tag3"
                />
              </div>
            )}
            {needsFileInput && (
              <div style={styles.field}>
                <label style={styles.label}>File name</label>
                <input
                  ref={fileRef}
                  style={styles.input}
                  value={fileName}
                  onChange={(e) => setFileName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      handleConfirm();
                    }
                  }}
                  placeholder="my-note.md"
                />
              </div>
            )}
          </div>
        </div>

        <div style={styles.footer}>
          <div style={styles.preview}>
            <span style={styles.previewLabel}>Preview:</span>
            <pre style={styles.previewContent}>
              {loadedContent.slice(0, 300)}
            </pre>
          </div>
          <div style={styles.actions}>
            <button style={styles.cancelBtn} onClick={onClose}>
              Cancel
            </button>
            <button style={styles.confirmBtn} onClick={handleConfirm}>
              Create Note
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default function TemplatePicker({ isOpen, onClose, onSelectTemplate, isTauri }: TemplatePickerProps) {
  if (!isOpen) return null;
  return (
    <TemplatePickerInner
      onClose={onClose}
      onSelectTemplate={onSelectTemplate}
      isTauri={isTauri}
    />
  );
}

const styles: Record<string, React.CSSProperties> = {
  overlay: {
    position: "fixed",
    inset: 0,
    backgroundColor: "rgba(0,0,0,0.6)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    zIndex: 1000,
  },
  dialog: {
    width: "520px",
    maxHeight: "80vh",
    backgroundColor: "#1e1e2e",
    borderRadius: "8px",
    border: "1px solid #313244",
    display: "flex",
    flexDirection: "column",
    outline: "none",
    boxShadow: "0 8px 32px rgba(0,0,0,0.5)",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "12px 16px",
    borderBottom: "1px solid #313244",
  },
  title: {
    fontSize: "14px",
    fontWeight: 600,
    color: "#cdd6f4",
  },
  closeBtn: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "24px",
    height: "24px",
    backgroundColor: "transparent",
    color: "#6c7086",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "14px",
  },
  body: {
    display: "flex",
    flex: 1,
    overflow: "hidden",
  },
  list: {
    width: "220px",
    minWidth: "180px",
    overflow: "auto",
    borderRight: "1px solid #313244",
    padding: "8px 4px",
    display: "flex",
    flexDirection: "column",
    gap: "2px",
  },
  item: {
    padding: "8px 10px",
    borderRadius: "4px",
    cursor: "pointer",
    display: "flex",
    flexDirection: "column",
    gap: "2px",
  },
  itemActive: {
    backgroundColor: "#313244",
  },
  itemTitle: {
    fontSize: "13px",
    color: "#cdd6f4",
    fontWeight: 500,
  },
  itemPreview: {
    fontSize: "10px",
    color: "#6c7086",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    maxWidth: "180px",
  },
  inputs: {
    flex: 1,
    padding: "12px 16px",
    display: "flex",
    flexDirection: "column",
    gap: "12px",
    overflow: "auto",
  },
  field: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
  },
  label: {
    fontSize: "11px",
    color: "#a6adc8",
    fontWeight: 500,
  },
  input: {
    padding: "6px 10px",
    fontSize: "13px",
    backgroundColor: "#11111b",
    color: "#cdd6f4",
    border: "1px solid #313244",
    borderRadius: "4px",
    outline: "none",
  },
  footer: {
    borderTop: "1px solid #313244",
    padding: "10px 16px",
    display: "flex",
    gap: "12px",
    alignItems: "flex-end",
  },
  preview: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    minWidth: 0,
  },
  previewLabel: {
    fontSize: "10px",
    color: "#6c7086",
    fontWeight: 500,
    textTransform: "uppercase",
  },
  previewContent: {
    margin: 0,
    padding: "4px 8px",
    fontSize: "11px",
    color: "#a6adc8",
    backgroundColor: "#11111b",
    borderRadius: "4px",
    border: "1px solid #313244",
    maxHeight: "60px",
    overflow: "hidden",
    whiteSpace: "pre-wrap",
    wordBreak: "break-word",
  },
  actions: {
    display: "flex",
    gap: "8px",
    flexShrink: 0,
  },
  cancelBtn: {
    padding: "6px 14px",
    fontSize: "12px",
    backgroundColor: "transparent",
    color: "#a6adc8",
    border: "1px solid #45475a",
    borderRadius: "4px",
    cursor: "pointer",
  },
  confirmBtn: {
    padding: "6px 14px",
    fontSize: "12px",
    backgroundColor: "#cba6f7",
    color: "#1e1e2e",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontWeight: 600,
  },
};
