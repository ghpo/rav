-- Base URL for external item images, configured per user alongside the
-- connected account. The item's image reference is appended to it.
ALTER TABLE external_folder_settings ADD COLUMN base_url TEXT;
