import { ChevronLeft, ChevronRight, Eye, MoreHorizontal, Plus, Search, Video } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { AppShell } from "../components/AppShell";
import { Button, IconButton } from "../components/ui";
import {
  banners,
  createBlankProject,
  createTemplateProject,
  imageCovers,
  templates,
  tvCategories
} from "../fixtures";
import { useStore } from "../storage";
import type { Project } from "../types";
import { formatDate } from "../utils";

export function HomePage() {
  const { projects, setProjects } = useStore();
  const navigate = useNavigate();
  const [slide, setSlide] = useState(1);
  const [category, setCategory] = useState("全部");
  const [query, setQuery] = useState("");
  const [categoryScroll, setCategoryScroll] = useState({ canLeft: false, canRight: false });
  const categoryRowRef = useRef<HTMLDivElement | null>(null);
  const currentBanner = banners[slide];
  const visibleBanners = [-1, 0, 1].map((offset) => {
    const index = (slide + offset + banners.length) % banners.length;
    return { ...banners[index], index, slot: offset };
  });

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
        <section className="hero-strip neumorphic-panel" aria-label="轮播推荐">
          <div className="hero-track">
            {visibleBanners.map((banner) => (
              <button
                key={`${banner.title}-${banner.slot}`}
                type="button"
                className={`hero-card ${banner.slot === 0 ? "active" : banner.slot < 0 ? "prev" : "next"}`}
                style={{ backgroundImage: `url(${banner.cover})` }}
                aria-label={banner.title}
                onClick={() => {
                  if (banner.slot === 0) {
                    createProject(currentBanner.tag === "文生视频");
                  } else {
                    setSlide(banner.index);
                  }
                }}
              >
                <span>{banner.tag}</span>
                <strong>{banner.title}</strong>
              </button>
            ))}
            <IconButton
              className="hero-arrow hero-prev"
              label="上一张"
              onClick={() => setSlide((value) => (value + banners.length - 1) % banners.length)}
            >
              <ChevronLeft size={18} />
            </IconButton>
            <IconButton
              className="hero-arrow hero-next"
              label="下一张"
              onClick={() => setSlide((value) => (value + 1) % banners.length)}
            >
              <ChevronRight size={18} />
            </IconButton>
          </div>
          <div className="hero-dots" role="tablist" aria-label="轮播分页">
            {banners.map((banner, index) => (
              <button
                key={banner.title}
                type="button"
                aria-label={`切换到 ${banner.title}`}
                className={index === slide ? "active" : ""}
                onClick={() => setSlide(index)}
              />
            ))}
          </div>
        </section>

        <section className="home-action-row" aria-label="创作入口">
          <Button className="home-create-btn" onClick={() => createProject(false)}>
            <Plus size={19} />
            开始创作
          </Button>
          <Button className="home-seedance-btn" onClick={() => createProject(true)}>
            <Video size={19} />
            <span>快速体验</span>
            <strong>Seedance 2.0</strong>
          </Button>
        </section>

        <section className="tv-show">
          <div className="section-head">
            <div>
              <h2>TV Show</h2>
            </div>
          </div>
          <div className="tv-filter-row">
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
              aria-label="TV Show 分类"
              ref={categoryRowRef}
              onScroll={updateCategoryScroll}
            >
              {tvCategories.map((item) => (
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
                aria-label="搜索 TV Show 模板"
                name="templateSearch"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="请输入搜索内容"
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
