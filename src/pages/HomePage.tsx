import { ChevronLeft, ChevronRight, Eye, MoreHorizontal, Plus, Search, Video } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { AppShell } from "../components/AppShell";
import { Button, IconButton } from "../components/ui";
import { createBlankProject, createTemplateProject, imageCovers, templateCategories, templates } from "../fixtures";
import { useStore } from "../storage";
import type { Project } from "../types";
import { formatDate } from "../utils";

export function HomePage() {
  const { projects, setProjects } = useStore();
  const navigate = useNavigate();
  const [category, setCategory] = useState("全部");
  const [query, setQuery] = useState("");
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
        <section className="home-hero" aria-label="创作工作台">
          <div className="home-hero-copy">
            <span className="home-hero-kicker">Local-first creative workspace</span>
            <h1>把脚本、镜头、素材和生成流程放在一个清晰的工作台里。</h1>
            <p>
              从想法、参考图到分镜与生成结果，全部保留在本地项目中，方便继续迭代、比较方案和推进交付。
            </p>
            <div className="home-hero-actions" aria-label="创作入口">
              <Button className="home-create-btn" onClick={() => createProject(false)}>
                <Plus size={19} />
                开始创作
              </Button>
              <Button className="home-seedance-btn" onClick={() => createProject(true)}>
                <Video size={19} />
                <span>快速体验</span>
                <strong>Seedance 2.0</strong>
              </Button>
            </div>
          </div>
          <div className="home-hero-preview" aria-hidden="true">
            <article className="hero-preview-card hero-preview-primary">
              <span>当前工作流</span>
              <strong>脚本整理 → 分镜参考 → 结果回看</strong>
              <p>围绕一个项目连续推进，而不是在多个页面之间来回切换。</p>
            </article>
            <div className="hero-preview-stack">
              <article className="hero-preview-card">
                <span>本地项目</span>
                <strong>{projects.length}</strong>
                <p>持续积累可复用的流程和素材。</p>
              </article>
              <article className="hero-preview-card hero-preview-accent">
                <span>推荐起点</span>
                <strong>Seedance 快速体验</strong>
                <p>适合先试镜头节奏，再回到完整项目继续细化。</p>
              </article>
            </div>
          </div>
        </section>

        <section className="template-library">
          <div className="section-head">
            <div>
              <h2>精选模板</h2>
              <p>按主题浏览现成工作流，快速打开一个可继续修改的创作过程。</p>
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

        {recentProjects.length > 0 && (
          <>
            <section className="section-head local-project-head">
              <div>
                <h2>本地项目</h2>
                <p>从最近打开的项目继续推进，不打断当前创作节奏。</p>
              </div>
              <Link to="/project" className="text-link">
                全部项目
              </Link>
            </section>
            <section className="recent-grid local-recent-grid">
              {recentProjects.map((project) => (
                <ProjectCard key={project.id} project={project} onOpen={() => navigate(`/canvas/${project.id}`)} />
              ))}
            </section>
          </>
        )}
      </main>
    </AppShell>
  );
}

function ProjectCard({ project, onOpen }: { project: Project; onOpen: () => void }) {
  const coverUrl = project.coverUrl || imageCovers[0];
  return (
    <article className="project-card" onClick={onOpen}>
      <img src={coverUrl} alt="" loading="lazy" />
      <div>
        <h3>{project.name}</h3>
        <span>{formatDate(project.updatedAt)}</span>
      </div>
      <IconButton label="项目菜单" onClick={(event) => event.stopPropagation()}>
        <MoreHorizontal size={16} />
      </IconButton>
    </article>
  );
}
