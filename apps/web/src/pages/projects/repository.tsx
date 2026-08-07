import hljs from "highlight.js/lib/common";
import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { Pagination } from "../../components/Pagination";
import { Icon } from "../../components/ui";
import type {
  CompanyProject,
  ProjectRepositoryFileResponse,
  ProjectRepositoryRefsResponse,
  ProjectRepositoryTreeResponse,
} from "../../types/platform";

const DIRECTORY_PAGE_SIZE = 100;

export function ProjectRepositoryBrowser(props: {
  companyId: string;
  project: CompanyProject;
  token: string;
  onError: (error: unknown) => void;
}) {
  const [refsResponse, setRefsResponse] = useState<ProjectRepositoryRefsResponse | null>(null);
  const [selectedRef, setSelectedRef] = useState("");
  const [currentPath, setCurrentPath] = useState("");
  const [tree, setTree] = useState<ProjectRepositoryTreeResponse | null>(null);
  const [selectedFile, setSelectedFile] = useState<ProjectRepositoryFileResponse | null>(null);
  const [page, setPage] = useState(1);
  const [loadingRefs, setLoadingRefs] = useState(false);
  const [loadingTree, setLoadingTree] = useState(false);
  const [loadingFile, setLoadingFile] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");

  async function loadRefs(preferredRef = selectedRef) {
    setLoadingRefs(true);
    setErrorMessage("");
    try {
      const response = await api<ProjectRepositoryRefsResponse>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/repository/refs`,
        {},
        props.token,
      );
      const nextRef = response.refs.some((reference) => reference.full_name === preferredRef)
        ? preferredRef
        : response.default_ref;
      setRefsResponse(response);
      setSelectedRef(nextRef);
      if (nextRef !== preferredRef) {
        setCurrentPath("");
        setPage(1);
        setSelectedFile(null);
      }
    } catch (error) {
      setErrorMessage(errorMessageOf(error));
      props.onError(error);
    } finally {
      setLoadingRefs(false);
    }
  }

  useEffect(() => {
    setRefsResponse(null);
    setSelectedRef("");
    setCurrentPath("");
    setTree(null);
    setSelectedFile(null);
    setPage(1);
    void loadRefs("");
    // Project identity is the reset boundary; loadRefs deliberately uses the fresh empty ref.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.companyId, props.project.project.id, props.token]);

  useEffect(() => {
    if (!selectedRef) return;
    let active = true;
    setLoadingTree(true);
    setErrorMessage("");
    const query = new URLSearchParams({
      ref: selectedRef,
      path: currentPath,
      page: String(page),
      per_page: String(DIRECTORY_PAGE_SIZE),
    });
    api<ProjectRepositoryTreeResponse>(
      `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/repository/tree?${query}`,
      {},
      props.token,
    )
      .then((response) => {
        if (active) setTree(response);
      })
      .catch((error) => {
        if (!active) return;
        setTree(null);
        setErrorMessage(errorMessageOf(error));
        props.onError(error);
      })
      .finally(() => {
        if (active) setLoadingTree(false);
      });
    return () => { active = false; };
  }, [currentPath, page, props.companyId, props.project.project.id, props.token, selectedRef]);

  function changeRef(reference: string) {
    setSelectedRef(reference);
    setCurrentPath("");
    setPage(1);
    setSelectedFile(null);
  }

  function openDirectory(path: string) {
    setCurrentPath(path);
    setPage(1);
    setSelectedFile(null);
  }

  async function openFile(path: string) {
    setLoadingFile(true);
    setErrorMessage("");
    try {
      const query = new URLSearchParams({ ref: selectedRef, path });
      const response = await api<ProjectRepositoryFileResponse>(
        `/api/v1/companies/${props.companyId}/projects/${props.project.project.id}/repository/file?${query}`,
        {},
        props.token,
      );
      setSelectedFile(response);
    } catch (error) {
      setErrorMessage(errorMessageOf(error));
      props.onError(error);
    } finally {
      setLoadingFile(false);
    }
  }

  const highlightedCode = useMemo(() => {
    if (!selectedFile?.content) return null;
    const language = highlightLanguage(selectedFile.language, selectedFile.name);
    if (!language || !hljs.getLanguage(language)) {
      return { language: "text", html: escapeHtml(selectedFile.content) };
    }
    return {
      language,
      html: hljs.highlight(selectedFile.content, { language, ignoreIllegals: true }).value,
    };
  }, [selectedFile]);

  if (!props.project.git) {
    return (
      <div className="project-repository-browser-empty">
        <Icon name="git" />
        <strong>Harness 仓库尚未初始化完成</strong>
      </div>
    );
  }

  const breadcrumbs = repositoryBreadcrumbs(currentPath);
  const selectedReference = refsResponse?.refs.find((reference) => reference.full_name === selectedRef);

  return (
    <article className="project-repository-browser">
      <header className="project-repository-browser-toolbar">
        <div className="project-repository-ref-control">
          <Icon name="git" />
          <select
            aria-label="浏览分支"
            value={selectedRef}
            disabled={loadingRefs || !refsResponse?.refs.length}
            onChange={(event) => changeRef(event.target.value)}
          >
            {(refsResponse?.refs ?? []).map((reference) => (
              <option value={reference.full_name} key={reference.full_name}>
                {reference.kind === "tag" ? "Tag · " : ""}{reference.name}{reference.is_default ? " · 默认" : ""}
              </option>
            ))}
          </select>
        </div>
        <div className="project-repository-browser-state">
          {refsResponse ? (
            <span className="repository-source harness_api">Harness API</span>
          ) : null}
          {selectedReference ? <code>{selectedReference.commit.slice(0, 10)}</code> : null}
          <button className="button small" type="button" disabled={loadingRefs} onClick={() => void loadRefs()}>
            <Icon name="refresh" /> {loadingRefs ? "刷新中…" : "刷新"}
          </button>
        </div>
      </header>

      {errorMessage ? <div className="project-repository-browser-error"><Icon name="alert" /> {errorMessage}</div> : null}

      <div className="project-repository-browser-grid">
        <section className="project-repository-browser-tree">
          <nav className="repository-breadcrumbs" aria-label="项目目录路径">
            {breadcrumbs.map((crumb, index) => (
              <span key={crumb.path || "root"}>
                {index ? <Icon name="chevron-right" /> : null}
                <button type="button" onClick={() => openDirectory(crumb.path)}>{crumb.label}</button>
              </span>
            ))}
          </nav>
          <div className="repository-entry-list" aria-busy={loadingTree}>
            {loadingTree && !tree ? <div className="repository-loading">读取目录…</div> : null}
            {!loadingTree && tree && !tree.entries.length ? <div className="repository-loading">空目录</div> : null}
            {tree?.entries.map((entry) => {
              const directory = entry.kind === "directory";
              const readable = directory || ["file", "symlink"].includes(entry.kind);
              return (
                <button
                  className={`${directory ? "directory" : "file"} ${selectedFile?.path === entry.path ? "active" : ""}`}
                  type="button"
                  disabled={!readable || loadingFile}
                  onClick={() => directory ? openDirectory(entry.path) : void openFile(entry.path)}
                  key={entry.path}
                >
                  <span className="repository-entry-icon"><Icon name={directory ? "folder" : "book"} /></span>
                  <span className="repository-entry-name"><strong>{entry.name}</strong><small>{entry.kind === "submodule" ? "Git Submodule" : entry.mode}</small></span>
                  {entry.size !== null ? <span className="repository-entry-size">{formatBytes(entry.size)}</span> : null}
                  <Icon name="chevron-right" />
                </button>
              );
            })}
          </div>
          {tree ? (
            <Pagination
              compact
              page={tree.page}
              pageCount={tree.total_pages}
              pageSize={tree.per_page}
              total={tree.total}
              onPageChange={setPage}
            />
          ) : null}
        </section>

        <section className="project-repository-browser-preview">
          {selectedFile ? (
            <>
              <header className="repository-file-header">
                <div><strong>{selectedFile.name}</strong><small>{selectedFile.path}</small></div>
                <div>
                  <span>{formatBytes(selectedFile.size)}</span>
                  {selectedFile.line_count !== null ? <span>{selectedFile.line_count} 行</span> : null}
                  <code>{selectedFile.commit.slice(0, 10)}</code>
                </div>
              </header>
              {selectedFile.binary || selectedFile.content === null ? (
                <div className="repository-file-unavailable"><Icon name="image" /><strong>该文件无法文本预览</strong></div>
              ) : (
                <div className="repository-code-frame">
                  <span className="repository-code-language">{highlightedCode?.language ?? "text"}</span>
                  <pre><code className={`hljs language-${highlightedCode?.language ?? "text"}`} dangerouslySetInnerHTML={{ __html: highlightedCode?.html ?? "" }} /></pre>
                </div>
              )}
            </>
          ) : (
            <div className="repository-preview-placeholder">
              <Icon name="book" />
              <strong>{loadingFile ? "读取文件…" : "选择文件查看内容"}</strong>
            </div>
          )}
        </section>
      </div>
    </article>
  );
}

function repositoryBreadcrumbs(path: string) {
  const parts = path.split("/").filter(Boolean);
  return [
    { label: "根目录", path: "" },
    ...parts.map((label, index) => ({ label, path: parts.slice(0, index + 1).join("/") })),
  ];
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function highlightLanguage(extension: string, name: string) {
  const normalized = extension.toLowerCase();
  const aliases: Record<string, string> = {
    cjs: "javascript", cpp: "cpp", cs: "csharp", dockerfile: "dockerfile", h: "c", hpp: "cpp",
    html: "xml", java: "java", js: "javascript", jsx: "javascript", json: "json", kt: "kotlin",
    kts: "kotlin", md: "markdown", mjs: "javascript", php: "php", ps1: "powershell", py: "python",
    rb: "ruby", rs: "rust", sh: "bash", sql: "sql", swift: "swift", toml: "ini", ts: "typescript",
    tsx: "typescript", vue: "xml", xml: "xml", yaml: "yaml", yml: "yaml", zsh: "bash",
  };
  if (/^dockerfile(?:\.|$)/iu.test(name)) return "dockerfile";
  return aliases[normalized] ?? normalized;
}

function escapeHtml(value: string) {
  return value
    .replace(/&/gu, "&amp;")
    .replace(/</gu, "&lt;")
    .replace(/>/gu, "&gt;")
    .replace(/"/gu, "&quot;")
    .replace(/'/gu, "&#039;");
}

function errorMessageOf(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
