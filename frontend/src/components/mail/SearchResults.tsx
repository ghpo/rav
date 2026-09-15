"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { AnimatedDiv } from "@/lib/motion/AnimatedDiv";
import { ArrowDown, ArrowUp, Check, Loader2, Mail, MailOpen, Paperclip, Star, Trash2, X, SearchX } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Chip } from "@/components/ui/Chip";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/useUiStore";
import { useSearch } from "@/hooks/useSearch";
import {
  useBulkUpdateFlags,
  useBulkMoveMessages,
  useBulkDeleteMessages,
} from "@/hooks/useMessages";
import { resolveSpecialFolderName } from "@/lib/folders";
import {
  getFilterLabel,
  isValidCommittedSearch,
  normalizeSearchQuery,
  parseSearchQuery,
  removeFilterFromQuery,
} from "@/lib/search-parser";
import type { SearchResultItem } from "@/types/message";

const resultKey = (r: Pick<SearchResultItem, "folder_name" | "uid">) =>
  `${r.folder_name}::${r.uid}`;

function formatDate(dateStr: string): string {
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return dateStr;

  const now = new Date();
  const isToday =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();

  if (isToday) {
    return date.toLocaleTimeString(undefined, {
      hour: "numeric",
      minute: "2-digit",
    });
  }

  const msPerDay = 86_400_000;
  const daysDiff = Math.floor((now.getTime() - date.getTime()) / msPerDay);
  const time = date.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });

  if (daysDiff < 7 && daysDiff >= 0) {
    const day = date.toLocaleDateString(undefined, { weekday: "short" });
    return `${day} ${time}`;
  }

  if (date.getFullYear() === now.getFullYear()) {
    const day = date.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
    return `${day} ${time}`;
  }

  const day = date.toLocaleDateString(undefined, {
    month: "2-digit",
    day: "2-digit",
    year: "2-digit",
  });
  return `${day} ${time}`;
}

function SearchResultRow({
  result,
  isSelected,
  isBulkSelected,
  onClick,
}: {
  result: SearchResultItem;
  isSelected: boolean;
  isBulkSelected: boolean;
  onClick: (e: React.MouseEvent<HTMLButtonElement>) => void;
}) {
  const sender = result.from_name || result.from_address;
  const formattedDate = formatDate(result.date);
  const isUnread = !result.flags.includes("\\Seen");
  const isFlagged = result.flags.includes("\\Flagged");

  return (
    <button
      type="button"
      onClick={onClick}
      data-search-result-folder={result.folder_name}
      data-search-result-uid={result.uid}
      className={cn(
        "flex w-full cursor-pointer flex-col gap-0.5 border-b border-border px-3 py-2 text-left transition-colors",
        "hover:bg-accent active:bg-accent/70",
        isUnread ? "bg-background" : "bg-transparent",
        isSelected && "bg-accent hover:bg-accent active:bg-accent/70",
        isBulkSelected && "bg-primary/10 hover:bg-primary/15 active:bg-primary/20",
      )}
    >
      {/* Top row: bulk checkbox or unread dot, sender, folder badge, date */}
      <div className="flex items-center gap-2">
        {isBulkSelected ? (
          <span className="flex size-4 shrink-0 items-center justify-center rounded border border-primary bg-primary text-primary-foreground">
            <Check className="size-3" />
          </span>
        ) : (
          <span
            className={cn(
              "size-1.5 shrink-0 rounded-full",
              isUnread ? "bg-primary" : "bg-transparent",
            )}
          />
        )}
        <span className={cn(
          "min-w-0 flex-1 truncate text-sm",
          isUnread ? "font-semibold" : "font-medium",
          isFlagged ? "text-primary" : "text-foreground",
        )}>
          {sender}
        </span>
        <span className="shrink-0 rounded bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
          {result.folder_name}
        </span>
        <span className={cn("shrink-0 text-xs", isFlagged ? "text-primary" : "text-muted-foreground")}>
          {formattedDate}
        </span>
      </div>

      {/* Subject + attachment */}
      <div className="flex items-center gap-2 pl-3.5">
        <span className={cn(
          "min-w-0 flex-1 truncate text-sm",
          isUnread ? "font-medium" : "font-normal",
          isFlagged ? "text-primary" : "text-foreground",
        )}>{result.subject || "(no subject)"}</span>
        {result.has_attachments && (
          <Paperclip className="size-3.5 shrink-0 text-muted-foreground" />
        )}
      </div>

      {/* Snippet */}
      {result.snippet && (
        <p className="truncate pl-3.5 text-xs text-muted-foreground">
          {result.snippet}
        </p>
      )}
    </button>
  );
}

export function SearchResults() {
  const searchQuery = useUiStore((s) => s.searchQuery);
  const setSearchQuery = useUiStore((s) => s.setSearchQuery);
  const setSearchActive = useUiStore((s) => s.setSearchActive);
  const setActiveFolder = useUiStore((s) => s.setActiveFolder);
  const selectMessage = useUiStore((s) => s.selectMessage);
  const activeFolder = useUiStore((s) => s.activeFolder);
  const selectedMessageUid = useUiStore((s) => s.selectedMessageUid);
  const searchSortOrder = useUiStore((s) => s.searchSortOrder);
  const setSearchSortOrder = useUiStore((s) => s.setSearchSortOrder);
  const setSearchResultCount = useUiStore((s) => s.setSearchResultCount);
  const listTransition = {
    initial: { opacity: 0, y: 6 },
    animate: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.22, ease: [0.2, 0, 0, 1] as const },
    },
    exit: {
      opacity: 0,
      y: 3,
      transition: { duration: 0.14, ease: [0.2, 0, 0, 1] as const },
    },
  };

  const itemTransition = {
    initial: { opacity: 0, x: 6 },
    animate: {
      opacity: 1,
      x: 0,
      transition: { duration: 0.18, ease: [0.2, 0, 0, 1] as const },
    },
    exit: {
      opacity: 0,
      x: -3,
      transition: { duration: 0.1, ease: [0.2, 0, 0, 1] as const },
    },
  };

  const normalizedSearchQuery = normalizeSearchQuery(searchQuery);
  const hasValidCommittedSearch = isValidCommittedSearch(normalizedSearchQuery);

  const {
    data,
    isLoading,
    isError,
    isFetchingNextPage,
    hasNextPage,
    fetchNextPage,
  } = useSearch(searchQuery, undefined, searchSortOrder);

  // Parse filters for display in the results header
  const parsed = parseSearchQuery(normalizedSearchQuery);

  // ---- Multi-selection (shift/cmd) across search results ----
  const queryClient = useQueryClient();
  const bulkUpdateFlags = useBulkUpdateFlags();
  const bulkMoveMessages = useBulkMoveMessages();
  const bulkDeleteMessages = useBulkDeleteMessages();
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());
  const [isBulkBusy, setIsBulkBusy] = useState(false);
  const anchorIndexRef = useRef<number | null>(null);

  const clearSelection = useCallback(() => {
    setSelectedKeys(new Set());
    anchorIndexRef.current = null;
  }, []);

  // Clear the selection whenever the query or ordering changes.
  useEffect(() => {
    clearSelection();
  }, [normalizedSearchQuery, searchSortOrder, clearSelection]);

  const handleRemoveFilter = useCallback(
    (filterRaw: string) => {
      const nextQuery = normalizeSearchQuery(removeFilterFromQuery(searchQuery, filterRaw));
      const hasValidNextQuery = isValidCommittedSearch(nextQuery);
      setSearchQuery(hasValidNextQuery ? nextQuery : "");
      setSearchActive(hasValidNextQuery);
    },
    [searchQuery, setSearchQuery, setSearchActive],
  );

  const scrollRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const prevSelectionKeyRef = useRef<string | null>(null);

  const results = useMemo(() => data?.pages.flatMap((p) => p.results) ?? [], [data?.pages]);
  const totalCount = data?.pages[0]?.total_count ?? 0;

  // Sync result count to the store so sibling components (e.g. SearchBar) can
  // read it without subscribing to the full query cache.
  useEffect(() => {
    setSearchResultCount(hasValidCommittedSearch ? totalCount : null);
    return () => setSearchResultCount(null);
  }, [totalCount, hasValidCommittedSearch, setSearchResultCount]);

  useEffect(() => {
    if (selectedMessageUid == null || results.length === 0) return;

    const selectionKey = `${activeFolder}:${selectedMessageUid}`;
    if (selectionKey === prevSelectionKeyRef.current) return;

    const scrollEl = scrollRef.current;
    if (!scrollEl) return;

    const selectedRow = scrollEl.querySelector(
      `[data-search-result-folder="${activeFolder}"][data-search-result-uid="${selectedMessageUid}"]`,
    ) as HTMLElement | null;
    if (!selectedRow) return;

    prevSelectionKeyRef.current = selectionKey;

    const scrollRect = scrollEl.getBoundingClientRect();
    const rowRect = selectedRow.getBoundingClientRect();
    const rowTop = rowRect.top - scrollRect.top + scrollEl.scrollTop;
    const rowBottom = rowTop + rowRect.height;
    const viewTop = scrollEl.scrollTop;
    const viewBottom = viewTop + scrollEl.clientHeight;
    const buffer = rowRect.height * 3;

    if (rowTop < viewTop + buffer) {
      scrollEl.scrollTop = Math.max(0, rowTop - buffer);
    } else if (rowBottom > viewBottom - buffer) {
      scrollEl.scrollTop = rowBottom - scrollEl.clientHeight + buffer;
    }
  }, [selectedMessageUid, activeFolder, results.length]);

  // rootMargin "30%" is 30% of the scroll container's visible height —
  // constant regardless of total content length, adapts to device size.
  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel || !hasNextPage) return;
    const observer = new IntersectionObserver(
      (entries) => { if (entries[0]?.isIntersecting) fetchNextPage(); },
      { root: scrollRef.current, rootMargin: "30%" },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasNextPage, fetchNextPage]);

  const handleRowClick = useCallback(
    (result: SearchResultItem, index: number, e: React.MouseEvent<HTMLButtonElement>) => {
      const isMod = e.metaKey || e.ctrlKey;
      const isShift = e.shiftKey;

      if (isMod) {
        e.preventDefault();
        setSelectedKeys((prev) => {
          const next = new Set(prev);
          const key = resultKey(result);
          if (next.has(key)) next.delete(key);
          else next.add(key);
          return next;
        });
        anchorIndexRef.current = index;
        return;
      }

      if (isShift && anchorIndexRef.current != null) {
        e.preventDefault();
        const start = Math.min(anchorIndexRef.current, index);
        const end = Math.max(anchorIndexRef.current, index);
        const next = new Set<string>();
        for (let i = start; i <= end; i++) {
          const r = results[i];
          if (r) next.add(resultKey(r));
        }
        setSelectedKeys(next);
        return;
      }

      // Plain click: clear bulk selection and open the message.
      clearSelection();
      anchorIndexRef.current = index;
      setActiveFolder(result.folder_name);
      selectMessage(result.uid);
    },
    [results, clearSelection, setActiveFolder, selectMessage],
  );

  // Selected uids grouped by their folder, since bulk endpoints are per-folder.
  const groups = useMemo(() => {
    const map = new Map<string, number[]>();
    for (const r of results) {
      if (selectedKeys.has(resultKey(r))) {
        const arr = map.get(r.folder_name) ?? [];
        arr.push(r.uid);
        map.set(r.folder_name, arr);
      }
    }
    return map;
  }, [results, selectedKeys]);

  const selectedCount = useMemo(() => {
    let n = 0;
    for (const arr of groups.values()) n += arr.length;
    return n;
  }, [groups]);

  const handleBulkFlags = useCallback(
    async (flags: string[], add: boolean) => {
      setIsBulkBusy(true);
      try {
        for (const [folder, uids] of groups) {
          await bulkUpdateFlags.mutateAsync({ folder, uids, flags, add });
        }
        clearSelection();
      } catch (err) {
        toast.error(`Falha na ação: ${(err as Error).message}`);
      } finally {
        setIsBulkBusy(false);
      }
    },
    [groups, bulkUpdateFlags, clearSelection],
  );

  const handleBulkDelete = useCallback(async () => {
    const trashName = resolveSpecialFolderName(queryClient, ["Trash"], "\\trash");
    if (!trashName) {
      toast.error("Pasta de lixeira não encontrada.");
      return;
    }
    if (
      selectedMessageUid != null &&
      selectedKeys.has(`${activeFolder}::${selectedMessageUid}`)
    ) {
      selectMessage(null);
    }
    setIsBulkBusy(true);
    try {
      for (const [folder, uids] of groups) {
        if (folder === trashName) {
          await bulkDeleteMessages.mutateAsync({ folder, uids });
        } else {
          await bulkMoveMessages.mutateAsync({
            fromFolder: folder,
            toFolder: trashName,
            uids,
          });
        }
      }
      clearSelection();
    } catch (err) {
      toast.error(`Falha ao excluir: ${(err as Error).message}`);
    } finally {
      setIsBulkBusy(false);
    }
  }, [
    queryClient,
    selectedMessageUid,
    activeFolder,
    selectedKeys,
    selectMessage,
    groups,
    bulkDeleteMessages,
    bulkMoveMessages,
    clearSelection,
  ]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* Loading state (initial load only) */}
      {isLoading && (
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="size-6 animate-spin text-muted-foreground" />
        </div>
      )}

      {/* Error state */}
      {isError && (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 px-4 text-center">
          <p className="text-sm text-muted-foreground">
            Failed to load search results
          </p>
        </div>
      )}

      {/* Empty state */}
      {!isLoading && !isError && hasValidCommittedSearch && results.length === 0 && (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 px-4 text-center">
          <SearchX className="size-10 text-muted-foreground/40" strokeWidth={1.25} />
          <div>
            <p className="text-sm font-medium text-muted-foreground">No results found</p>
            <p className="mt-0.5 text-xs text-muted-foreground/70">Try different keywords or check spelling</p>
          </div>
        </div>
      )}

      {/* Results list with infinite scroll */}
      {!isLoading && !isError && hasValidCommittedSearch && (
        <>
          {/* Result count header */}
          {results.length > 0 && (
            <div className="shrink-0 border-b border-border px-3 py-1">
              <div className="flex items-center justify-between">
                <span className="text-xs text-muted-foreground">
                  {results.length < totalCount
                    ? `Showing ${results.length} of ${totalCount} results`
                    : `${totalCount} result${totalCount !== 1 ? "s" : ""}`}
                </span>
                <button
                  type="button"
                  onClick={() => setSearchSortOrder(searchSortOrder === "date_desc" ? "date_asc" : "date_desc")}
                  className="flex items-center gap-1 rounded px-1.5 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-accent active:bg-accent/70 hover:text-foreground"
                  title={searchSortOrder === "date_desc" ? "Newest first" : "Oldest first"}
                >
                  {searchSortOrder === "date_desc" ? (
                    <ArrowDown className="size-3" />
                  ) : (
                    <ArrowUp className="size-3" />
                  )}
                  Date
                </button>
              </div>
              {parsed.filters.length > 0 && (
                <div className="mt-1 flex flex-wrap gap-1">
                  {parsed.filters.map((filter, idx) => (
                    <Chip
                      key={`${filter.operator}-${idx}`}
                      onRemove={() => handleRemoveFilter(filter.raw)}
                      removeLabel={`Remove ${filter.operator} filter`}
                    >
                      {getFilterLabel(filter)}
                    </Chip>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Bulk action bar (appears when results are multi-selected) */}
          {selectedCount > 0 && (
            <div className="flex shrink-0 flex-wrap items-center gap-1 border-b border-border bg-muted/50 px-2 py-1">
              <span className="mr-1 text-xs font-medium">
                {selectedCount} selecionado{selectedCount !== 1 ? "s" : ""}
              </span>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1 px-2 text-xs"
                title="Marcar como lido"
                disabled={isBulkBusy}
                onClick={() => handleBulkFlags(["\\Seen"], true)}
              >
                <MailOpen className="size-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1 px-2 text-xs"
                title="Marcar como não lido"
                disabled={isBulkBusy}
                onClick={() => handleBulkFlags(["\\Seen"], false)}
              >
                <Mail className="size-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1 px-2 text-xs"
                title="Favoritar"
                disabled={isBulkBusy}
                onClick={() => handleBulkFlags(["\\Flagged"], true)}
              >
                <Star className="size-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1 px-2 text-xs text-destructive hover:text-destructive"
                title="Excluir"
                disabled={isBulkBusy}
                onClick={handleBulkDelete}
              >
                <Trash2 className="size-3.5" />
              </Button>
              {isBulkBusy ? (
                <Loader2 className="size-3.5 animate-spin text-muted-foreground" />
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs"
                  title="Limpar seleção"
                  onClick={clearSelection}
                >
                  <X className="size-3.5" />
                </Button>
              )}
            </div>
          )}

          <AnimatePresence initial={false}>
            {results.length > 0 && (
              <AnimatedDiv
                key="search-results-list"
                ref={scrollRef}
                data-testid="search-results-list-transition"
                variants={listTransition}
                initial={listTransition.initial}
                animate={listTransition.animate}
                exit={listTransition.exit}
                className="min-h-0 flex-1 overflow-y-auto"
              >
                <AnimatePresence initial={false}>
                  {results.map((result, index) => (
                    <AnimatedDiv
                      key={`${result.folder_name}-${result.uid}`}
                      data-testid="search-results-item-transition"
                      variants={itemTransition}
                      initial={itemTransition.initial}
                      animate={itemTransition.animate}
                      exit={itemTransition.exit}
                    >
                      <SearchResultRow
                        result={result}
                        isSelected={
                          activeFolder === result.folder_name &&
                          selectedMessageUid === result.uid
                        }
                        isBulkSelected={selectedKeys.has(resultKey(result))}
                        onClick={(e) => handleRowClick(result, index, e)}
                      />
                    </AnimatedDiv>
                  ))}
                </AnimatePresence>
                <div ref={sentinelRef} className="h-px" />
                {isFetchingNextPage && (
                  <div className="flex justify-center py-3">
                    <Loader2 className="size-4 animate-spin text-muted-foreground" />
                  </div>
                )}
              </AnimatedDiv>
            )}
          </AnimatePresence>
        </>
      )}
    </div>
  );
}
