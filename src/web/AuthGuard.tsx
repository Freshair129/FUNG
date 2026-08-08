import { useEffect, useState } from "react";
import { supabase } from "../lib/supabase";
import { LoadingScreen } from "./LoadingScreen";

type AuthGuardProps = {
  children: React.ReactNode;
};

export function AuthGuard({ children }: AuthGuardProps) {
  const [state, setState] = useState<"loading" | "authenticated" | "unauthenticated">("loading");

  useEffect(() => {
    supabase.auth.getSession().then(({ data }) => {
      setState(data.session ? "authenticated" : "unauthenticated");
    });

    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((_event, session) => {
      setState(session ? "authenticated" : "unauthenticated");
    });

    return () => subscription.unsubscribe();
  }, []);

  if (state === "loading") {
    return <LoadingScreen />;
  }

  if (state === "unauthenticated") {
    window.location.href = "/";
    return null;
  }

  return <>{children}</>;
}
