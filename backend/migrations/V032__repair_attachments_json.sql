-- Repair `attachments_json` rows written by older builds (notably the
-- background deep-index phase) that serialized the raw `ImapAttachment`
-- shape. That JSON has no `id` field and embeds the binary attachment bytes
-- under `"data":[...]`, so it cannot be deserialized as `AttachmentMeta` and
-- the UI silently showed no attachments.
--
-- Clearing these rows (and forcing a body re-fetch) lets the fixed writer
-- repopulate the cache with the correct metadata-only shape. The `"data"` key
-- never appears in a valid `AttachmentMeta` array.
UPDATE messages
SET attachments_json = NULL,
    body_cached = 0,
    has_attachments = 1
WHERE attachments_json LIKE '%"data":%';
