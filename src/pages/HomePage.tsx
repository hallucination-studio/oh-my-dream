import {
  Archive,
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  Eye,
  FolderOpen,
  HardDrive,
  MoreHorizontal,
  Plus,
  Search,
  Settings,
  Upload,
  Video
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { AppShell } from "../components/AppShell";
import { WorkspaceStatusModal } from "../components/WorkspaceStatusModal";
import { Button, IconButton } from "../components/ui";
import { createBlankProject, createTemplateProject, imageCovers, templateCategories, templates } from "../fixtures";
import { useStore } from "../storage";
import { formatDate } from "../utils";

export function HomePage() {
  const { projects, setProjects, assets, tasks, config } = useStore();
  const navigate = useNavigate();
  const [category, setCategory] = useState("全部");
  const [query, setQuery] = useState("");
  const [templatesOpen, setTemplatesOpen] = useState(false);
  const [workspaceOpen, setWorkspaceOpen] = useState(false);
  const [categoryScroll, setCategoryScroll] = useState({ canLeft: false, canRight: false });
  const categoryRowRef = useRef<HTMLDivElement | null>(null);

  const recentProjects = useMemo(
    () =>
      [...projects]
        .filter((project) => !project.readonly)
        .sort((a, b) => +new Date(b.updatedAt) - +new Date(a.updatedAt))
        .slice(0, 5),
    [projects]
  );
  const activeTasks = useMemo(
    () => tasks.filter((task) => task.status === "queued" || task.status === "running").slice(0, 4),
    [tasks]
  );
  const failedTasks = useMemo(() => tasks.filter((task) => task.status === "failed"), [tasks]);
  const providerReady =
    (config.providers.openai.enabled && Boolean(config.providers.openai.apiKey)) ||
    (config.providers.volcengineArk.enabled && Boolean(config.providers.volcengineArk.apiKey));

  const filteredTemplates = useMemo(() => {
    const keyword = query.trim().toLowerCase();
    return templates.filter((template) => {
      const categoryMatch = category === "全部" || template.category === category;
      const keywordMatch =
        !keyword ||
        template.title.toLowerCase().includes(keyword) ||
        template.author.toLowerCase().includes(keyword);
      return categoryMatch && keywordMatch;
    });
  }, [category, query]);

  const createProject = useCallback(
    (seedance = false) => {
      const project = createBlankProject(seedance ? "Seedance2.0 未命名" : "未命名", seedance);
      setProjects((items) => [project, ...items]);
      navigate(`/canvas/${project.id}`);
    },
    [navigate, setProjects]
  );

  const useTemplate = useCallback(
    (templateId: string, readonly = false) => {
      const project = createTemplateProject(templateId, readonly);
      setProjects((items) => [project, ...items]);
      navigate(`/canvas/${project.id}`);
    },
    [navigate, setProjects]
  );

  const updateCategoryScroll = useCallback(() => {
    const row = categoryRowRef.current;
    if (!row) {
      return;
    }
    setCategoryScroll({
      canLeft: row.scrollLeft > 1,
      canRight: row.scrollLeft + row.clientWidth < row.scrollWidth - 1
    });
  }, []);

  useEffect(() => {
    updateCategoryScroll();
    const onResize = () => updateCategoryScroll();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [updateCategoryScroll]);

  const scrollCategories = useCallback(
    (left: number) => {
      categoryRowRef.current?.scrollBy({ left, behavior: "smooth" });
      window.setTimeout(updateCategoryScroll, 260);
    },
    [updateCategoryScroll]
  );

  return (
    <AppShell>
      <main className="home-main">
        <section className="workbench-hero" aria-label="本地工作台">
          <div className="workbench-copy">
            <span className="workbench-kicker">
              <CheckCircle2 size={14} />
              本地工作台
            </span>
            <h1>继续推进视频项目</h1>
            <p>把项目、素材、提示词和生成记录放在同一处，打开后直接回到最近的制作上下文。</p>
          </div>
          <div className="workbench-actions" aria-label="工作区操作">
            <Button variant="primary" onClick={() => createProject(false)}>
              <Plus size={18} />
              创建项目
            </Button>
            <Button onClick={() => setWorkspaceOpen(true)}>
              <Upload size={18} />
              导入备份
            </Button>
            <Button onClick={() => navigate("/project")}>
              <FolderOpen size={18} />
              打开项目库
            </Button>
          </div>
        </section>

        <section className="workbench-grid" aria-label="工作台概览">
          <article className="continue-panel">
            <div className="panel-head">
              <div>
                <h2>继续工作</h2>
                <p>{recentProjects.length > 0 ? "最近修改的本地项目" : "创建第一个本地项目，画布会自动保存到当前设备"}</p>
              </div>
              <Link to="/project" className="text-link">
                全部项目
              </Link>
            </div>
            <div className="continue-list">
              {recentProjects.length > 0 ? (
                recentProjects.slice(0, 4).map((project) => (
                  <button key={project.id} type="button" className="continue-row" onClick={() => navigate(`/canvas/${project.id}`)}>
                    <img src={project.coverUrl || imageCovers[0]} alt="" loading="lazy" />
                    <span>
                      <strong>{project.name}</strong>
                      <small>{project.workspacePath ?? `workspace/${project.id}`} · {formatDate(project.updatedAt)}</small>
                    </span>
                    <ChevronRight size={16} />
                  </button>
                ))
              ) : (
                <button type="button" className="empty-workbench-action" onClick={() => createProject(false)}>
                  <Plus size={18} />
                  新建空白项目
                </button>
              )}
            </div>
          </article>

          <aside className="workspace-health-panel" aria-label="工作区健康度">
            <div className="panel-head">
              <div>
                <h2>工作区状态</h2>
                <p>本地适配器与生成配置概览</p>
              </div>
              <Button size="sm" onClick={() => setWorkspaceOpen(true)}>
                <Archive size={15} />
                备份
              </Button>
            </div>
            <div className="health-grid">
              <article>
                <HardDrive size={16} />
                <span>项目</span>
                <strong>{projects.filter((project) => !project.readonly).length}</strong>
              </article>
              <article>
                <span>资产</span>
                <strong>{assets.length}</strong>
              </article>
              <article className={providerReady ? "is-ok" : "is-warning"}>
                <span>Provider</span>
                <strong>{providerReady ? "可用" : "未配置"}</strong>
              </article>
              <article className={activeTasks.length > 0 ? "is-running" : ""}>
                <span>任务</span>
                <strong>{activeTasks.length}</strong>
              </article>
            </div>
            <div className="task-summary-list">
              {activeTasks.length > 0 ? (
                activeTasks.map((task) => (
                  <div key={task.id}>
                    <span>{task.title}</span>
                    <progress value={task.progress} max={100} />
                  </div>
                ))
              ) : (
                <p>{failedTasks.length > 0 ? `${failedTasks.length} 个失败任务可在画布历史中检查。` : "没有正在运行的生成任务。"}</p>
              )}
            </div>
            <Button onClick={() => navigate("/config")}>
              <Settings size={16} />
              打开设置
            </Button>
          </aside>
        </section>

        <section className="starter-strip" aria-label="快速起步">
          <Button onClick={() => createProject(true)}>
            <Video size={17} />
            Seedance 视频草稿
          </Button>
          <Button onClick={() => setTemplatesOpen((value) => !value)}>
            <Archive size={17} />
            {templatesOpen ? "收起模板" : "浏览起步模板"}
          </Button>
        </section>

        {templatesOpen && (
          <section className="template-library starter-template-library">
            <div className="section-head">
              <div>
                <h2>起步模板</h2>
                <p>模板以只读方式打开，用于参考流程；创建副本后再进入正式项目。</p>
              </div>
            </div>
            <div className="template-filter-row">
              <IconButton
                className="category-scroll-prev"
                label="向左滚动"
                disabled={!categoryScroll.canLeft}
                onClick={() => scrollCategories(-220)}
              >
                <ChevronLeft size={18} />
              </IconButton>
              <div
                className="tab-row"
                role="tablist"
                aria-label="模板分类"
                ref={categoryRowRef}
                onScroll={updateCategoryScroll}
              >
                {templateCategories.map((item) => (
                  <button
                    key={item}
                    type="button"
                    className={item === category ? "active" : ""}
                    onClick={() => setCategory(item)}
                  >
                    {item}
                  </button>
                ))}
              </div>
              <IconButton
                className="category-scroll-next"
                label="向右滚动"
                disabled={!categoryScroll.canRight}
                onClick={() => scrollCategories(220)}
              >
                <ChevronRight size={18} />
              </IconButton>
              <label className="search-box">
                <Search size={16} />
                <input
                  aria-label="搜索模板"
                  name="templateSearch"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="搜索标题或作者"
                />
              </label>
            </div>
            <div className="template-grid">
              {filteredTemplates.map((template) => (
                <article className="template-card" key={template.id} onClick={() => useTemplate(template.id, true)}>
                  <div className="template-cover">
                    <img src={template.cover} alt="" loading="lazy" />
                    <span className="template-views">
                      <Eye size={13} />
                      {template.views}
                    </span>
                    {template.award && <em className="template-award">{template.award}</em>}
                    <Button
                      size="sm"
                      className="template-process"
                      onClick={(event) => {
                        event.stopPropagation();
                        useTemplate(template.id, true);
                      }}
                    >
                      查看创作过程
                    </Button>
                  </div>
                  <div className="template-meta">
                    {template.avatar ? (
                      <img className="template-avatar-img" src={template.avatar} alt={template.author} loading="lazy" />
                    ) : (
                      <span className="template-avatar">{template.author.slice(0, 1).toUpperCase()}</span>
                    )}
                    <p>{template.author}</p>
                    {template.tier && (
                      <span className={`template-tier ${template.tier === "专业" ? "pro" : ""}`}>
                        {template.tier}
                      </span>
                    )}
                  </div>
                  <div className="template-title-row">
                    <h3>{template.title}</h3>
                    <IconButton label="模板菜单" onClick={(event) => event.stopPropagation()}>
                      <MoreHorizontal size={14} />
                    </IconButton>
                  </div>
                </article>
              ))}
            </div>
          </section>
        )}
      </main>
      {workspaceOpen && <WorkspaceStatusModal onClose={() => setWorkspaceOpen(false)} />}
    </AppShell>
  );
}
