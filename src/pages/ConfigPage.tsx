import { ArrowLeft } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { AppShell } from "../components/AppShell";
import { ConfigModal } from "../components/ConfigModal";
import { Button } from "../components/ui";

export function ConfigPage() {
  const navigate = useNavigate();
  return (
    <AppShell>
      <main className="config-page">
        <Button onClick={() => navigate(-1)}>
          <ArrowLeft size={16} />
          返回
        </Button>
        <section className="standalone-config">
          <ConfigModal onClose={() => navigate("/")} />
        </section>
      </main>
    </AppShell>
  );
}
