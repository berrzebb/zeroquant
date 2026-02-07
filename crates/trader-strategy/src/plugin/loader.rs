//! 동적 전략 플러그인 로더.
//!
//! 동적 라이브러리(Windows의 .dll, Linux의 .so)에서 전략 플러그인을 로드합니다.
//! 플러그인 업데이트 시 핫 리로딩을 지원합니다.

use crate::Strategy;
use libloading::Library;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// 플러그인 로더 에러.
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("라이브러리 로드 실패: {0}")]
    LoadError(String),

    #[error("심볼을 찾을 수 없음: {0}")]
    SymbolNotFound(String),

    #[error("플러그인을 찾을 수 없음: {0}")]
    PluginNotFound(String),

    #[error("유효하지 않은 플러그인: {0}")]
    InvalidPlugin(String),

    #[error("이미 로드된 플러그인: {0}")]
    AlreadyLoaded(String),

    #[error("IO 에러: {0}")]
    IoError(#[from] std::io::Error),
}

/// 플러그인이 반환하는 플러그인 메타데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// 플러그인 이름
    pub name: String,
    /// 플러그인 버전
    pub version: String,
    /// 플러그인 설명
    pub description: String,
    /// 필수 설정 키
    pub required_config: Vec<String>,
    /// 지원되는 심볼 (비어있으면 전체)
    pub supported_tickers: Vec<String>,
}

// 참고: 이러한 FFI 타입은 엄격히 FFI 안전하지 않은 트레이트 객체를 사용하지만,
// 양쪽이 동일한 메모리 레이아웃과 ABI를 사용하는 Rust-to-Rust 플러그인 로딩에서는
// 실제로 동작합니다.
#[allow(improper_ctypes_definitions)]
/// create_strategy 함수 시그니처에 대한 타입 별칭.
type CreateStrategyFn = unsafe extern "C" fn() -> *mut dyn Strategy;

#[allow(improper_ctypes_definitions)]
/// destroy_strategy 함수 시그니처에 대한 타입 별칭.
#[allow(dead_code)] // 향후 플러그인 정리 구현을 위해 예약됨
type DestroyStrategyFn = unsafe extern "C" fn(*mut dyn Strategy);

#[allow(improper_ctypes_definitions)]
/// get_metadata 함수 시그니처에 대한 타입 별칭.
type GetMetadataFn = unsafe extern "C" fn() -> PluginMetadata;

/// 로드된 플러그인.
pub struct LoadedPlugin {
    /// 플러그인 파일 경로
    path: PathBuf,

    /// 로드된 라이브러리
    library: Library,

    /// 플러그인 메타데이터
    metadata: PluginMetadata,
}

impl LoadedPlugin {
    /// 파일 경로에서 플러그인 로드.
    ///
    /// # Safety
    ///
    /// 호출자는 다음을 보장해야 합니다:
    /// - 경로가 동일한 Rust 버전으로 빌드된 유효한 동적 라이브러리를 가리킴
    /// - 라이브러리가 올바른 시그니처를 가진 `create_strategy` 및 `get_metadata` 함수를 내보냄
    /// - 라이브러리가 호환 가능한 메모리 레이아웃으로 컴파일됨 (동일한 Rust 컴파일러 버전)
    pub unsafe fn load<P: AsRef<Path>>(path: P) -> Result<Self, PluginError> {
        let path = path.as_ref().to_path_buf();

        info!(path = %path.display(), "Loading plugin");

        let library = Library::new(&path)
            .map_err(|e| PluginError::LoadError(format!("{}: {}", path.display(), e)))?;

        // Try to get metadata (optional)
        let metadata = if let Ok(get_metadata) =
            library.get::<libloading::Symbol<GetMetadataFn>>(b"get_metadata")
        {
            get_metadata()
        } else {
            // Default metadata if not provided
            PluginMetadata {
                name: path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                version: "1.0.0".to_string(),
                description: "Strategy plugin".to_string(),
                required_config: Vec::new(),
                supported_tickers: Vec::new(),
            }
        };

        // Verify required tickers exist
        let _: libloading::Symbol<CreateStrategyFn> = library
            .get(b"create_strategy")
            .map_err(|_| PluginError::SymbolNotFound("create_strategy".to_string()))?;

        info!(
            name = %metadata.name,
            version = %metadata.version,
            "Plugin loaded successfully"
        );

        Ok(Self {
            path,
            library,
            metadata,
        })
    }

    /// 이 플러그인에서 새 전략 인스턴스 생성.
    ///
    /// # Safety
    ///
    /// 호출자는 다음을 보장해야 합니다:
    /// - 플러그인이 `LoadedPlugin::load`를 통해 올바르게 로드됨
    /// - 반환된 `Box<dyn Strategy>`가 스레드 안전한 방식으로 사용됨
    pub unsafe fn create_strategy(&self) -> Result<Box<dyn Strategy>, PluginError> {
        let create_fn: libloading::Symbol<CreateStrategyFn> = self
            .library
            .get(b"create_strategy")
            .map_err(|_| PluginError::SymbolNotFound("create_strategy".to_string()))?;

        let raw_ptr = create_fn();
        if raw_ptr.is_null() {
            return Err(PluginError::InvalidPlugin(
                "create_strategy returned null".to_string(),
            ));
        }

        Ok(Box::from_raw(raw_ptr))
    }

    /// 플러그인 메타데이터 반환.
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    /// 플러그인 경로 반환.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// 플러그인 설정.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    /// 플러그인 파일 경로 (plugins 디렉토리 기준 상대 경로)
    pub path: String,

    /// 플러그인 활성화 여부
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// 전략 설정
    #[serde(default)]
    pub config: serde_json::Value,

    /// 파일 변경 시 자동 리로드
    #[serde(default)]
    pub hot_reload: bool,
}

fn default_true() -> bool {
    true
}

/// 플러그인 로더 설정.
#[derive(Debug, Clone, Deserialize)]
pub struct LoaderConfig {
    /// 플러그인을 검색할 디렉토리
    #[serde(default = "default_plugins_dir")]
    pub plugins_dir: PathBuf,

    /// 모든 플러그인에 대해 핫 리로드 활성화
    #[serde(default)]
    pub hot_reload: bool,

    /// 검색할 플러그인 파일 확장자
    #[serde(default = "default_extension")]
    pub extension: String,
}

fn default_plugins_dir() -> PathBuf {
    PathBuf::from("plugins")
}

fn default_extension() -> String {
    #[cfg(target_os = "windows")]
    {
        "dll".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "so".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "dylib".to_string()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        "so".to_string()
    }
}

impl Default for LoaderConfig {
    fn default() -> Self {
        Self {
            plugins_dir: default_plugins_dir(),
            hot_reload: false,
            extension: default_extension(),
        }
    }
}

/// 전략 플러그인을 동적으로 로드하기 위한 플러그인 로더.
pub struct PluginLoader {
    /// 로더 설정
    config: LoaderConfig,

    /// 이름별 로드된 플러그인
    plugins: Arc<RwLock<HashMap<String, LoadedPlugin>>>,
}

impl PluginLoader {
    /// 새 플러그인 로더 생성.
    pub fn new(config: LoaderConfig) -> Self {
        Self {
            config,
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 파일에서 플러그인 로드.
    pub async fn load_plugin<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<PluginMetadata, PluginError> {
        let path = path.as_ref();

        // Resolve relative paths
        let full_path = if path.is_relative() {
            self.config.plugins_dir.join(path)
        } else {
            path.to_path_buf()
        };

        if !full_path.exists() {
            return Err(PluginError::LoadError(format!(
                "Plugin file not found: {}",
                full_path.display()
            )));
        }

        let plugin = unsafe { LoadedPlugin::load(&full_path)? };
        let metadata = plugin.metadata().clone();
        let name = metadata.name.clone();

        let mut plugins = self.plugins.write().await;

        if plugins.contains_key(&name) {
            return Err(PluginError::AlreadyLoaded(name));
        }

        plugins.insert(name, plugin);

        Ok(metadata)
    }

    /// 이름으로 플러그인 언로드.
    pub async fn unload_plugin(&self, name: &str) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;

        plugins
            .remove(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;

        info!(plugin = %name, "Plugin unloaded");

        Ok(())
    }

    /// 로드된 플러그인에서 전략 인스턴스 생성.
    pub async fn create_strategy(
        &self,
        plugin_name: &str,
    ) -> Result<Box<dyn Strategy>, PluginError> {
        let plugins = self.plugins.read().await;

        let plugin = plugins
            .get(plugin_name)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

        unsafe { plugin.create_strategy() }
    }

    /// 로드된 플러그인의 메타데이터 반환.
    pub async fn get_plugin_metadata(&self, name: &str) -> Result<PluginMetadata, PluginError> {
        let plugins = self.plugins.read().await;

        let plugin = plugins
            .get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;

        Ok(plugin.metadata().clone())
    }

    /// 모든 로드된 플러그인 목록 반환.
    pub async fn list_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins.values().map(|p| p.metadata().clone()).collect()
    }

    /// 플러그인 디렉토리를 스캔하고 모든 플러그인 로드.
    pub async fn scan_and_load(&self) -> Result<Vec<PluginMetadata>, PluginError> {
        let plugins_dir = &self.config.plugins_dir;

        if !plugins_dir.exists() {
            info!(path = %plugins_dir.display(), "Creating plugins directory");
            std::fs::create_dir_all(plugins_dir)?;
            return Ok(Vec::new());
        }

        let extension = OsStr::new(&self.config.extension);
        let mut loaded = Vec::new();

        for entry in std::fs::read_dir(plugins_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension() == Some(extension) {
                match self.load_plugin(&path).await {
                    Ok(metadata) => {
                        info!(
                            plugin = %metadata.name,
                            version = %metadata.version,
                            "Loaded plugin from directory scan"
                        );
                        loaded.push(metadata);
                    }
                    Err(e) => {
                        warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to load plugin"
                        );
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// 이름으로 플러그인 리로드.
    pub async fn reload_plugin(&self, name: &str) -> Result<PluginMetadata, PluginError> {
        let path = {
            let plugins = self.plugins.read().await;
            let plugin = plugins
                .get(name)
                .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;
            plugin.path().to_path_buf()
        };

        // Unload and reload
        self.unload_plugin(name).await?;
        self.load_plugin(&path).await
    }

    /// 플러그인이 로드되었는지 확인.
    pub async fn is_loaded(&self, name: &str) -> bool {
        let plugins = self.plugins.read().await;
        plugins.contains_key(name)
    }

    /// 로드된 플러그인 수 반환.
    pub async fn plugin_count(&self) -> usize {
        let plugins = self.plugins.read().await;
        plugins.len()
    }
}

/// 내장 전략 생성을 위한 팩토리.
pub struct BuiltinStrategyFactory;

impl BuiltinStrategyFactory {
    /// 이름으로 내장 전략 생성.
    pub fn create(name: &str) -> Option<Box<dyn Strategy>> {
        match name.to_lowercase().as_str() {
            "grid" | "grid_trading" | "grid trading" => {
                Some(Box::new(crate::strategies::DcaStrategy::grid()))
            }
            "rsi" | "rsi_mean_reversion" | "rsi mean reversion" => {
                Some(Box::new(crate::strategies::MeanReversionStrategy::rsi()))
            }
            _ => None,
        }
    }

    /// 사용 가능한 내장 전략 목록 반환.
    pub fn list() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "grid_trading",
                "Grid Trading - Places buy/sell orders at regular intervals",
            ),
            (
                "rsi_mean_reversion",
                "RSI Mean Reversion - Buys oversold, sells overbought",
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_config_defaults() {
        let config = LoaderConfig::default();

        assert_eq!(config.plugins_dir, PathBuf::from("plugins"));
        assert!(!config.hot_reload);

        #[cfg(target_os = "windows")]
        assert_eq!(config.extension, "dll");

        #[cfg(target_os = "linux")]
        assert_eq!(config.extension, "so");
    }

    #[test]
    fn test_builtin_factory() {
        let grid = BuiltinStrategyFactory::create("grid");
        assert!(grid.is_some());
        // DcaStrategy 통합 전략은 variant별 이름 반환
        assert_eq!(grid.unwrap().name(), "DCA-Grid");

        let rsi = BuiltinStrategyFactory::create("rsi");
        assert!(rsi.is_some());
        assert_eq!(rsi.unwrap().name(), "MeanReversion-RSI");

        let unknown = BuiltinStrategyFactory::create("unknown");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_builtin_list() {
        let list = BuiltinStrategyFactory::list();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_plugin_loader_creation() {
        let loader = PluginLoader::new(LoaderConfig::default());

        assert_eq!(loader.plugin_count().await, 0);
    }
}
