"use client";

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiGet, apiPut } from "@/lib/api";

export interface CustomNavLink {
  /** Target URL. Empty string means no shortcut button is shown. */
  url: string;
  /** Icon key resolved by `getCustomLinkIcon`. */
  icon: string;
}

interface UpdateCustomNavLink {
  url?: string;
  icon?: string;
}

export function useCustomNavLink() {
  return useQuery({
    queryKey: ["custom-nav-link"],
    queryFn: () => apiGet<CustomNavLink>("/settings/custom-link"),
    staleTime: 5 * 60 * 1000,
  });
}

export function useUpdateCustomNavLink() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: UpdateCustomNavLink) =>
      apiPut<CustomNavLink>(
        "/settings/custom-link",
        data as Record<string, unknown>,
      ),
    onSuccess: (result) => {
      queryClient.setQueryData(["custom-nav-link"], result);
    },
  });
}
