import { useEffect } from "react";

import { startLinkBridge, useLinkStore } from "../lib/link/linkStore";
import { usePlanStore } from "../lib/plan/planStore";
import { AppShell } from "./layout/AppShell";

/**
 * The root application component.
 *
 * The subscription to the state of the link is started here, once: every part of the
 * interface reads the state from the store rather than opening its own subscription.
 */
export function App() {
  const status = useLinkStore((state) => state.status);
  const vehicles = useLinkStore((state) => state.vehicles.length);
  const plan = usePlanStore((state) => state.plans.mission);
  const download = usePlanStore((state) => state.download);

  useEffect(() => {
    startLinkBridge();
  }, []);

  // The plan the vehicle holds is read as soon as the vehicle has answered, so that the
  // map has something to draw and the actions panel can name the items. The link can be
  // open a moment before there is a vehicle on it, so the plan is read when the vehicle
  // appears rather than when the link opens.
  useEffect(() => {
    if (status === "connected" && vehicles > 0 && plan === null) {
      void download("mission");
    }
  }, [status, vehicles, plan, download]);

  return <AppShell />;
}
