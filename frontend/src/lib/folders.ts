import type { QueryClient } from "@tanstack/react-query";
import type { FolderId } from "@/types/folder";
import type { FoldersResponse } from "@/types/folder";

/**
 * Resolve a folder name to its current FolderId by reading the warm
 * `["folders"]` query cache synchronously - no network request, safe to call
 * from inside a queryFn/mutationFn (unlike a hook, which cannot be called
 * outside render).
 *
 * FolderIds are single-use and non-deterministic (a fresh one is minted on
 * every server response), so this is the only correct way to go from a
 * folder *name* to a usable id. If you already have a record (Folder,
 * MessageHeader, MessageDetail, SearchResultItem) with its own `folder_id`,
 * use that directly instead - it's already valid and this lookup is
 * unnecessary.
 *
 * Throws if the folders list hasn't loaded yet or the name doesn't match any
 * known folder, rather than silently sending a request with "undefined" in
 * the URL.
 */
export function resolveFolderId(queryClient: QueryClient, name: string): FolderId {
  const folders = queryClient.getQueryData<FoldersResponse>(["folders"])?.folders;
  const id = folders?.find((f) => f.name === name)?.id;
  if (!id) {
    throw new Error(`Unknown folder "${name}" - folder list not loaded or folder does not exist`);
  }
  return id;
}

/**
 * Resolve a special folder's real name by IMAP special-use attribute first
 * (e.g. `\Trash`), falling back to a case-insensitive name match. Returns
 * `null` when the folder list is not loaded or has no match.
 *
 * This avoids hardcoding localized/server-specific names (a server may call
 * the trash folder "Deleted Items" or similar) while still targeting a real
 * folder that `resolveFolderId` can resolve.
 */
export function resolveSpecialFolderName(
  queryClient: QueryClient,
  candidates: string[],
  attribute: string,
): string | null {
  const folders = queryClient.getQueryData<FoldersResponse>(["folders"])?.folders;
  if (!folders) return null;

  const attr = attribute.toLowerCase();
  const byAttribute = folders.find((f) =>
    f.attributes?.some((a) => a.toLowerCase() === attr),
  );
  if (byAttribute) return byAttribute.name;

  const lowered = candidates.map((c) => c.toLowerCase());
  const byName = folders.find((f) => lowered.includes(f.name.toLowerCase()));
  return byName?.name ?? null;
}
