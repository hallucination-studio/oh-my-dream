import { ArrowLeft, Archive, ArrowDownAZ, Ellipsis, Folder, FolderPlus, Plus, Search, Upload } from "lucide-react";
import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { AppShell } from "../components/AppShell";
import { WorkspaceStatusModal } from "../components/WorkspaceStatusModal";
import { Button, IconButton, Modal } from "../components/ui";
import { createBlankProject, imageCovers } from "../fixtures";
import { useStore } from "../storage";
import { formatDate } from "../utils";

export function ProjectPage() {
  const {
    projects,
    setProjects,
    ui,
    createFolder,
    duplicateProject,
    deleteProject,
    updateProject
  } = useStore();
  const navigate = useNavigate();
  const [folderView, setFolderView] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<"updated" | "name" | "created">("updated");
  const [menuId, setMenuId] = useState<string | null>(null);
  const [folderModal, setFolderModal] = useState(false);
  const [workspaceOpen, setWorkspaceOpen] = useState(false);
  const [renameId, setRenameId] = useState<string | null>(null);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [moveId, setMoveId] = useState<string | null>(null);
  const visibleProjects = useMemo(() => {
    const keyword = query.trim().toLowerCase();
    return projects
      .filter((project) => !project.readonly && (folderView ? project.folderId === folderView : !project.folderId))
      .filter((project) => {
        if (!keyword) {
          return true;
        }
        return (
          project.name.toLowerCase().includes(keyword) ||
          (project.workspacePath ?? "").toLowerCase().includes(keyword)
        );
      })
      .sort((a, b) => {
        if (sort === "name") {
          return a.name.localeCompare(b.name, "zh-CN");
        }
        if (sort === "created") {
          return +new Date(b.createdAt) - +new Date(a.createdAt);
        }
        return +new Date(b.updatedAt) - +new Date(a.updatedAt);
      });
  }, [folderView, projects, query, sort]);
  const folder = ui.folders.find((item) => item.id === folderView);

  const createProject = () => {
    const project = createBlankProject("未命名");
    setProjects((items) => [{ ...project, folderId: folderView ?? undefined }, ...items]);
    navigate(`/canvas/${project.id}`);
  };

  return (
    <AppShell>
      <main className="project-main">
        <div className="project-title-row">
          <div>
            <Link to="/" className="back-link">
              <ArrowLeft size={16} />
              返回
            </Link>
            <h1>{folder ? folder.name : "全部项目"}</h1>
            <p className="project-title-meta">浏览器本地工作区 · {visibleProjects.length} 个项目</p>
          </div>
          <div className="row-actions">
            {folderView && (
              <Button onClick={() => setFolderView(null)}>
                <Folder size={16} />
                全部项目
              </Button>
            )}
            <Button onClick={() => setFolderModal(true)}>
              <FolderPlus size={16} />
              新建文件夹
            </Button>
            <Button onClick={() => setWorkspaceOpen(true)}>
              <Archive size={16} />
              备份/导入
            </Button>
          </div>
        </div>

        <section className="project-toolbar" aria-label="项目筛选">
          <label className="search-box">
            <Search size={16} />
            <input
              aria-label="搜索项目"
              name="projectSearch"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索项目名或工作区路径"
            />
          </label>
          <label className="project-sort">
            <ArrowDownAZ size={16} />
            <select
              aria-label="项目排序"
              name="projectSort"
              value={sort}
              onChange={(event) => setSort(event.target.value as "updated" | "name" | "created")}
            >
              <option value="updated">最近修改</option>
              <option value="created">创建时间</option>
              <option value="name">名称</option>
            </select>
          </label>
          <Button onClick={() => setWorkspaceOpen(true)}>
            <Upload size={16} />
            导入工作区
          </Button>
        </section>

        {!folderView && ui.folders.length > 0 && (
          <section className="folder-grid" aria-label="文件夹">
            {ui.folders.map((item) => (
              <button key={item.id} type="button" className="folder-card" onClick={() => setFolderView(item.id)}>
                <Folder size={22} />
                <strong>{item.name}</strong>
                <span>{projects.filter((project) => project.folderId === item.id).length} 个项目</span>
              </button>
            ))}
          </section>
        )}

        <section className="project-grid">
          <button type="button" className="start-card" onClick={createProject}>
            <Plus size={24} />
            <strong>开始创作</strong>
            <span>默认项目名：未命名</span>
          </button>
          {visibleProjects.map((project) => (
            <article className="project-card project-card-menu" key={project.id}>
              <button type="button" className="project-open" onClick={() => navigate(`/canvas/${project.id}`)}>
                <img src={project.coverUrl || imageCovers[0]} alt="" loading="lazy" />
                <span>
                  <strong>{project.name}</strong>
                  <small>{project.workspacePath ?? `workspace/${project.id}`}</small>
                  <small>{project.nodes.length} 节点 · {project.edges.length} 连接 · {formatDate(project.updatedAt)}</small>
                </span>
              </button>
              <IconButton
                label={`${project.name} 菜单`}
                onClick={() => setMenuId((value) => (value === project.id ? null : project.id))}
              >
                <Ellipsis size={16} />
              </IconButton>
              {menuId === project.id && (
                <div className="project-menu">
                  <button type="button" onClick={() => navigate(`/canvas/${project.id}`)}>
                    打开
                  </button>
                  <button type="button" onClick={() => setRenameId(project.id)}>
                    重命名
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      const copy = duplicateProject(project.id);
                      setMenuId(null);
                      if (copy) {
                        navigate(`/canvas/${copy.id}`);
                      }
                    }}
                  >
                    复制
                  </button>
                  <button type="button" onClick={() => setMoveId(project.id)}>
                    移动到文件夹
                  </button>
                  <button type="button" className="danger-text" onClick={() => setDeleteId(project.id)}>
                    删除
                  </button>
                </div>
              )}
            </article>
          ))}
        </section>
        {visibleProjects.length === 0 ? (
          <section className="project-empty-state">
            <strong>{query ? "没有匹配的项目" : "当前文件夹没有项目"}</strong>
            <p>{query ? "清空搜索词或切换文件夹后再试。" : "创建项目后会出现在这里，并保留工作区路径和最近修改状态。"}</p>
          </section>
        ) : (
          <p className="end-state">没有更多了</p>
        )}
      </main>
      {workspaceOpen && <WorkspaceStatusModal onClose={() => setWorkspaceOpen(false)} />}
      {folderModal && (
        <TextInputModal
          title="新建文件夹"
          label="文件夹名"
          initialValue="新建文件夹"
          onClose={() => setFolderModal(false)}
          onSubmit={(name) => {
            createFolder(name);
            setFolderModal(false);
          }}
        />
      )}
      {renameId && (
        <TextInputModal
          title="重命名项目"
          label="项目名"
          initialValue={projects.find((project) => project.id === renameId)?.name ?? ""}
          onClose={() => setRenameId(null)}
          onSubmit={(name) => {
            updateProject(renameId, { name });
            setRenameId(null);
            setMenuId(null);
          }}
        />
      )}
      {moveId && (
        <MoveProjectModal
          projectId={moveId}
          onClose={() => setMoveId(null)}
          onMove={(folderId) => {
            updateProject(moveId, { folderId });
            setMoveId(null);
            setMenuId(null);
          }}
        />
      )}
      {deleteId && (
        <ConfirmModal
          title="删除项目"
          body="删除后会从首页最近项目和项目列表中同步消失。导出的工作区备份不会被自动更新。"
          danger
          onClose={() => setDeleteId(null)}
          onConfirm={() => {
            deleteProject(deleteId);
            setDeleteId(null);
            setMenuId(null);
          }}
        />
      )}
    </AppShell>
  );
}

function TextInputModal({
  title,
  label,
  initialValue,
  onClose,
  onSubmit
}: {
  title: string;
  label: string;
  initialValue: string;
  onClose: () => void;
  onSubmit: (value: string) => void;
}) {
  const [value, setValue] = useState(initialValue);
  return (
    <Modal title={title} onClose={onClose} width={420}>
      <form
        className="stack-form"
        onSubmit={(event) => {
          event.preventDefault();
          const next = value.trim();
          if (next) {
            onSubmit(next);
          }
        }}
      >
        <label htmlFor="project-text-input-value">
          <span>{label}</span>
          <input
            id="project-text-input-value"
            name="textInputValue"
            value={value}
            onChange={(event) => setValue(event.target.value)}
            autoFocus
          />
        </label>
        <div className="modal-actions">
          <Button type="button" onClick={onClose}>
            取消
          </Button>
          <Button type="submit" variant="primary">
            保存
          </Button>
        </div>
      </form>
    </Modal>
  );
}

function ConfirmModal({
  title,
  body,
  danger,
  onClose,
  onConfirm
}: {
  title: string;
  body: string;
  danger?: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  return (
    <Modal title={title} onClose={onClose} width={430}>
      <p className="confirm-copy">{body}</p>
      <div className="modal-actions">
        <Button onClick={onClose}>取消</Button>
        <Button variant={danger ? "danger" : "primary"} onClick={onConfirm}>
          确认
        </Button>
      </div>
    </Modal>
  );
}

function MoveProjectModal({
  projectId,
  onClose,
  onMove
}: {
  projectId: string;
  onClose: () => void;
  onMove: (folderId?: string) => void;
}) {
  const { ui, projects } = useStore();
  const project = projects.find((item) => item.id === projectId);
  const [folderId, setFolderId] = useState(project?.folderId ?? "");
  return (
    <Modal title="移动到文件夹" onClose={onClose} width={430}>
      <div className="stack-form">
        <label htmlFor="move-project-folder">
          <span>目标文件夹</span>
          <select
            id="move-project-folder"
            name="targetFolder"
            value={folderId}
            onChange={(event) => setFolderId(event.target.value)}
          >
            <option value="">全部项目</option>
            {ui.folders.map((folder) => (
              <option key={folder.id} value={folder.id}>
                {folder.name}
              </option>
            ))}
          </select>
        </label>
        <div className="modal-actions">
          <Button onClick={onClose}>取消</Button>
          <Button variant="primary" onClick={() => onMove(folderId || undefined)}>
            移动
          </Button>
        </div>
      </div>
    </Modal>
  );
}
