import { BrowserRouter, Route, Routes } from "react-router-dom";
import { CanvasPage } from "./pages/CanvasPage";
import { ConfigPage } from "./pages/ConfigPage";
import { HomePage } from "./pages/HomePage";
import { ProjectPage } from "./pages/ProjectPage";
import { StoreProvider } from "./storage";

export function App() {
  return (
    <BrowserRouter>
      <StoreProvider>
        <Routes>
          <Route path="/" element={<HomePage />} />
          <Route path="/project" element={<ProjectPage />} />
          <Route path="/canvas/:projectId" element={<CanvasPage />} />
          <Route path="/config" element={<ConfigPage />} />
        </Routes>
      </StoreProvider>
    </BrowserRouter>
  );
}
