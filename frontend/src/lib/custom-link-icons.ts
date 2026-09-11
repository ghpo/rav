import {
  Printer,
  Link,
  Globe,
  Calendar,
  Folder,
  FileText,
  ShoppingCart,
  Star,
  Tag,
  Building2,
  Mail,
  ExternalLink,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

export interface CustomLinkIconOption {
  key: string;
  label: string;
  Icon: LucideIcon;
}

/** Icons the user can pick for the navigation-rail shortcut button. */
export const CUSTOM_LINK_ICONS: CustomLinkIconOption[] = [
  { key: "printer", label: "Impressora", Icon: Printer },
  { key: "link", label: "Link", Icon: Link },
  { key: "globe", label: "Site", Icon: Globe },
  { key: "calendar", label: "Agenda", Icon: Calendar },
  { key: "folder", label: "Pasta", Icon: Folder },
  { key: "file-text", label: "Documento", Icon: FileText },
  { key: "cart", label: "Loja", Icon: ShoppingCart },
  { key: "star", label: "Favorito", Icon: Star },
  { key: "tag", label: "Etiqueta", Icon: Tag },
  { key: "building", label: "Empresa", Icon: Building2 },
  { key: "mail", label: "E-mail", Icon: Mail },
  { key: "external", label: "Externo", Icon: ExternalLink },
];

/** Resolve a stored icon key to a component, falling back to a generic link. */
export function getCustomLinkIcon(key: string | null | undefined): LucideIcon {
  return CUSTOM_LINK_ICONS.find((option) => option.key === key)?.Icon ?? Link;
}
