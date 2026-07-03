import {
  Cloud,
  Router,
  Server,
  Network,
  Box,
  type LucideIcon,
} from "lucide-react";

// iconFor maps a catalog icon identifier to a Lucide icon component.
export function iconFor(icon?: string): LucideIcon {
  switch (icon) {
    case "router":
      return Router;
    case "switch":
      return Network;
    case "cloud":
      return Cloud;
    case "server":
      return Server;
    default:
      return Box;
  }
}
