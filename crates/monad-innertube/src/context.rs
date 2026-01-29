//! `InnerTube` client context configuration.

use serde::{Deserialize, Serialize};

/// Client context sent with every `InnerTube` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientContext {
    pub client: Client,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
}

impl ClientContext {
    /// Create a new client context for `YouTube` Music web client.
    pub fn music_web() -> Self {
        Self {
            client: Client::music_web(),
            user: None,
        }
    }

    /// Create a new client context for `YouTube` Music Android client.
    pub fn music_android() -> Self {
        Self {
            client: Client::music_android(),
            user: None,
        }
    }

    /// Create a new client context for `YouTube` Music iOS client.
    pub fn music_ios() -> Self {
        Self {
            client: Client::music_ios(),
            user: None,
        }
    }

    /// Set the user for authenticated requests.
    pub const fn with_user(mut self, user: User) -> Self {
        self.user = Some(user);
        self
    }
}

impl Default for ClientContext {
    fn default() -> Self {
        Self::music_web()
    }
}

/// Client information for `InnerTube` requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    /// Client name (e.g., "`WEB_REMIX`" for `YouTube` Music).
    pub client_name: String,
    /// Client version string.
    pub client_version: String,
    /// Platform (e.g., "DESKTOP").
    pub platform: Option<String>,
    /// User agent string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Locale/language (e.g., "en").
    pub hl: String,
    /// Geographic location (e.g., "US").
    pub gl: String,
    /// Timezone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    /// UTC offset in minutes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utc_offset_minutes: Option<i32>,
    /// Device make (for mobile clients).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_make: Option<String>,
    /// Device model (for mobile clients).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_model: Option<String>,
    /// OS name (for mobile clients).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_name: Option<String>,
    /// OS version (for mobile clients).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    /// Android SDK version (for Android client).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android_sdk_version: Option<u32>,
}

impl Client {
    /// `YouTube` Music web client (`WEB_REMIX`).
    pub fn music_web() -> Self {
        Self {
            client_name: "WEB_REMIX".to_string(),
            client_version: "1.20241106.01.00".to_string(),
            platform: Some("DESKTOP".to_string()),
            user_agent: Some(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string()
            ),
            hl: "en".to_string(),
            gl: "US".to_string(),
            time_zone: Some("America/New_York".to_string()),
            utc_offset_minutes: Some(-300),
            device_make: None,
            device_model: None,
            os_name: None,
            os_version: None,
            android_sdk_version: None,
        }
    }

    /// `YouTube` Music Android client.
    pub fn music_android() -> Self {
        Self {
            client_name: "ANDROID_MUSIC".to_string(),
            client_version: "6.42.52".to_string(),
            platform: Some("MOBILE".to_string()),
            user_agent: Some(
                "com.google.android.apps.youtube.music/6.42.52 (Linux; U; Android 11) gzip"
                    .to_string(),
            ),
            hl: "en".to_string(),
            gl: "US".to_string(),
            time_zone: None,
            utc_offset_minutes: None,
            device_make: Some("Google".to_string()),
            device_model: Some("Pixel 5".to_string()),
            os_name: Some("Android".to_string()),
            os_version: Some("11".to_string()),
            android_sdk_version: Some(30),
        }
    }

    /// `YouTube` Music iOS client.
    pub fn music_ios() -> Self {
        Self {
            client_name: "IOS_MUSIC".to_string(),
            client_version: "6.42".to_string(),
            platform: Some("MOBILE".to_string()),
            user_agent: Some(
                "com.google.ios.youtubemusic/6.42 (iPhone14,3; U; CPU iOS 17_0 like Mac OS X)"
                    .to_string(),
            ),
            hl: "en".to_string(),
            gl: "US".to_string(),
            time_zone: None,
            utc_offset_minutes: None,
            device_make: Some("Apple".to_string()),
            device_model: Some("iPhone14,3".to_string()),
            os_name: Some("iOS".to_string()),
            os_version: Some("17.0".to_string()),
            android_sdk_version: None,
        }
    }

    /// Get the numeric client ID for this client type.
    pub fn client_id(&self) -> u32 {
        match self.client_name.as_str() {
            "WEB_REMIX" => 67,
            "ANDROID_MUSIC" => 21,
            "IOS_MUSIC" => 26,
            _ => 67,
        }
    }

    /// Get the API key for this client type.
    pub fn api_key(&self) -> &'static str {
        match self.client_name.as_str() {
            "WEB_REMIX" => "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30",
            "ANDROID_MUSIC" => "AIzaSyAOghZGza2MQSZkY_zfZ370N-PUdXEo8AI",
            "IOS_MUSIC" => "AIzaSyBAETezhkwP0ZWA02RsqT1zu78Fpt0bC_s",
            _ => "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30",
        }
    }
}

/// User information for authenticated requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    /// Whether the user is logged in.
    pub logged_in: bool,
}

impl User {
    pub const fn logged_in() -> Self {
        Self { logged_in: true }
    }

    pub const fn anonymous() -> Self {
        Self { logged_in: false }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_client_context_serialization() {
        let ctx = ClientContext::music_web();
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("WEB_REMIX"));
    }

    #[test]
    fn test_client_api_keys() {
        assert_eq!(
            Client::music_web().api_key(),
            "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30"
        );
    }
}
