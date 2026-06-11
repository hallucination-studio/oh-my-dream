import { lazy, Suspense } from "react";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import { StoreProvider } from "./storage";

const HomePage = lazy(() => import("./pages/HomePage").then((module) => ({ default: module.HomePage })));
const ProjectPage = lazy(() => import("./pages/ProjectPage").then((module) => ({ default: module.ProjectPage })));
const CanvasPage = lazy(() => import("./pages/CanvasPage").then((module) => ({ default: module.CanvasPage })));
const ConfigPage = lazy(() => import("./pages/ConfigPage").then((module) => ({ default: module.ConfigPage })));

export function App() {
  return (
    <BrowserRouter>
      <StoreProvider>
        <Suspense fallback={<main className="route-loading">正在打开本地工作区...</main>}>
          <Routes>
            <Route path="/" element={<HomePage />} />
            <Route path="/project" element={<ProjectPage />} />
            <Route path="/canvas/:projectId" element={<CanvasPage />} />
            <Route path="/config" element={<ConfigPage />} />
          </Routes>
        </Suspense>
      </StoreProvider>
    </BrowserRouter>
  );
}
