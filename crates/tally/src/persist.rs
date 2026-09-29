//! Key/value persistence for [`crate::store`]. Values are serialized as
//! JSON under a string key. Two backends, chosen at compile time:
//! browser localStorage on wasm, files in the app data directory
//! everywhere else (on Android that's the app-private files dir).

use serde::{Serialize, de::DeserializeOwned};

#[cfg(target_family = "wasm")]
mod backend {
    use super::*;
    use gloo_storage::{LocalStorage, Storage};

    pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
        LocalStorage::get(key).ok()
    }

    pub fn set<T: Serialize>(key: &str, value: &T) {
        // Quota exhaustion is the only realistic failure; nowhere to
        // report it, same policy as the pre-split code.
        let _ = LocalStorage::set(key, value);
    }
}

#[cfg(not(target_family = "wasm"))]
mod backend {
    use super::*;
    use std::path::PathBuf;

    pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
        let text = std::fs::read_to_string(path_for(key)).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn set<T: Serialize>(key: &str, value: &T) {
        let path = path_for(key);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string(value) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Keys may contain `/` (e.g. "habits/v2"); flatten to one file name.
    fn path_for(key: &str) -> PathBuf {
        data_dir().join(format!("{}.json", key.replace('/', "-")))
    }

    #[cfg(target_os = "android")]
    fn data_dir() -> PathBuf {
        android_files_dir().unwrap_or_else(|| PathBuf::from("."))
    }

    /// The app-private files dir, via JNI: `context.getFilesDir()`.
    #[cfg(target_os = "android")]
    fn android_files_dir() -> Option<PathBuf> {
        crate::android::with_env(|env, context| {
            let files_dir = env
                .call_method(context, "getFilesDir", "()Ljava/io/File;", &[])?
                .l()?;
            let path: jni::objects::JString = env
                .call_method(&files_dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?
                .l()?
                .into();
            let path: String = env.get_string(&path)?.into();
            Ok(PathBuf::from(path))
        })
    }

    #[cfg(not(target_os = "android"))]
    fn data_dir() -> PathBuf {
        std::env::var_os("TALLY_DATA_DIR")
            .map(Into::into)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
    backend::get(key)
}

pub fn set<T: Serialize>(key: &str, value: &T) {
    backend::set(key, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_backend_round_trips() {
        let dir = std::env::temp_dir().join(format!("tally-persist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: the only test (and only code) mutating the environment.
        unsafe { std::env::set_var("TALLY_DATA_DIR", &dir) };

        assert_eq!(get::<Vec<u32>>("habits/test"), None);
        set("habits/test", &vec![1u32, 2, 3]);
        assert_eq!(get::<Vec<u32>>("habits/test"), Some(vec![1, 2, 3]));

        // The key's slash was flattened into the file name.
        assert!(dir.join("habits-test.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
