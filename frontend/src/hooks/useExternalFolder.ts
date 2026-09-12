"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPost, apiPut } from "@/lib/api";
import { useFolders } from "@/hooks/useFolders";

/** Attribute the backend sets on folders backed by the external integration. */
export const EXTERNAL_FOLDER_ATTR = "\\external";

export interface ExternalFolderStatus {
  configured: boolean;
  available: boolean;
  folder_name: string | null;
  enabled: boolean;
  connected: boolean;
  system: number | null;
  account_email: string | null;
  base_url: string | null;
  connected_at: string | null;
  count: number;
}

interface ConnectExternalFolderRequest {
  usuario: string;
  senha: string;
  base_url?: string | null;
}

interface UpdateExternalFolderRequest {
  enabled: boolean;
}

export function useExternalFolderSettings() {
  return useQuery({
    queryKey: ["external-folder"],
    queryFn: () => apiGet<ExternalFolderStatus>("/settings/external-folder"),
    staleTime: 60_000,
  });
}

export function useConnectExternalFolder() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: ConnectExternalFolderRequest) =>
      apiPost<ExternalFolderStatus>(
        "/settings/external-folder/connect",
        data as unknown as Record<string, unknown>,
      ),
    onSuccess: (result) => {
      queryClient.setQueryData(["external-folder"], result);
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    },
  });
}

export function useUpdateExternalFolder() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: UpdateExternalFolderRequest) =>
      apiPut<ExternalFolderStatus>(
        "/settings/external-folder",
        data as unknown as Record<string, unknown>,
      ),
    onSuccess: (result) => {
      queryClient.setQueryData(["external-folder"], result);
      queryClient.invalidateQueries({ queryKey: ["folders"] });
    },
  });
}

/** Names of the folders exposed by the external SQL integration. */
export function useExternalFolderNames(): Set<string> {
  const { data } = useFolders();
  const names = new Set<string>();
  for (const folder of data?.folders ?? []) {
    if (folder.attributes?.some((a) => a.toLowerCase() === EXTERNAL_FOLDER_ATTR)) {
      names.add(folder.name);
    }
  }
  return names;
}

/** Whether the given folder is backed by the external integration. */
export function useIsExternalFolder(name: string): boolean {
  const names = useExternalFolderNames();
  return names.has(name);
}
