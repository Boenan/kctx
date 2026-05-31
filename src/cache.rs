use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_TTL_SECS: u64 = 300; // 5 minutes

/// Represents the structure of the cached namespace data.
#[derive(Serialize, Deserialize)]
pub struct NamespaceCache {
    pub namespaces: Vec<String>,
    pub timestamp: u64,
}

impl NamespaceCache {
    /// Returns the platform-specific cache directory for kctx.
    fn get_cache_dir() -> Result<PathBuf> {
        let cache_dir = if let Some(val) = std::env::var_os("KCTX_CACHE_DIR") {
            PathBuf::from(val)
        } else {
            dirs::cache_dir()
                .context("Could not find cache directory")?
                .join("kctx")
        };

        if !cache_dir.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = fs::DirBuilder::new();
                builder.recursive(true).mode(0o700);
                builder
                    .create(&cache_dir)
                    .context("Failed to create cache directory with restricted permissions")?;
            }
            #[cfg(not(unix))]
            {
                fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;
            }
        }

        Ok(cache_dir)
    }

    /// Generates a safe file path for a given context name within the cache directory.
    fn get_cache_path(context_name: &str) -> Result<PathBuf> {
        let safe_name = context_name.replace(['/', ':'], "_");
        Ok(Self::get_cache_dir()?.join(format!("{}.json", safe_name)))
    }

    /// Attempts to load the namespace list for a specific context from the disk cache.
    /// Returns None if the cache is missing or expired (older than 5 minutes).
    pub fn load(context_name: &str) -> Result<Option<Vec<String>>> {
        let path = Self::get_cache_path(context_name)?;
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path).context("Failed to read cache file")?;
        let cache: NamespaceCache =
            serde_json::from_str(&content).context("Failed to parse cache")?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        if now - cache.timestamp > CACHE_TTL_SECS {
            return Ok(None); // Cache expired
        }

        Ok(Some(cache.namespaces))
    }

    /// Saves a list of namespaces to the disk cache for the specified context.
    pub fn save(context_name: &str, namespaces: Vec<String>) -> Result<()> {
        let path = Self::get_cache_path(context_name)?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let cache = NamespaceCache {
            namespaces,
            timestamp: now,
        };

        let content = serde_json::to_string(&cache).context("Failed to serialize cache")?;
        fs::write(path, content).context("Failed to write cache file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cache_save_and_load() -> Result<()> {
        let tmp_dir = tempdir()?;
        unsafe {
            std::env::set_var("KCTX_CACHE_DIR", tmp_dir.path());
        }

        let context = "test-context";
        let namespaces = vec!["ns1".to_string(), "ns2".to_string()];

        NamespaceCache::save(context, namespaces.clone())?;
        let loaded = NamespaceCache::load(context)?;

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap(), namespaces);

        Ok(())
    }

    #[test]
    fn test_cache_expiration() -> Result<()> {
        let tmp_dir = tempdir()?;
        unsafe {
            std::env::set_var("KCTX_CACHE_DIR", tmp_dir.path());
        }

        let context = "expired-context";
        let namespaces = vec!["ns1".to_string()];

        // Save with an old timestamp
        let path = NamespaceCache::get_cache_path(context)?;
        let cache = NamespaceCache {
            namespaces,
            timestamp: 0, // Very old
        };
        let content = serde_json::to_string(&cache)?;
        fs::write(path, content)?;

        let loaded = NamespaceCache::load(context)?;
        assert!(loaded.is_none());

        Ok(())
    }

    #[test]
    fn test_cache_isolation() -> Result<()> {
        let tmp_dir = tempdir()?;
        unsafe {
            std::env::set_var("KCTX_CACHE_DIR", tmp_dir.path());
        }

        NamespaceCache::save("ctx1", vec!["ns1".to_string()])?;
        NamespaceCache::save("ctx2", vec!["ns2".to_string()])?;

        let load1 = NamespaceCache::load("ctx1")?.unwrap();
        let load2 = NamespaceCache::load("ctx2")?.unwrap();

        assert_eq!(load1, vec!["ns1"]);
        assert_eq!(load2, vec!["ns2"]);

        Ok(())
    }
}
